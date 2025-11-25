use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{execute, terminal};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Terminal;

use serde::Serialize;

use globset::GlobBuilder;
use wasm_poke::{
    build_call_graph, disassemble_function_wat_lines, function_matches, parse_wasm,
    unique_cumulative_size, CallGraph, FunctionInfo, WasmModuleInfo, WatLine,
};

#[derive(Debug, Parser)]
#[command(
    name = "wasm-poke",
    version,
    about = "Interactive WebAssembly function size explorer"
)]
struct Cli {
    /// Path to the .wasm file
    wasm_path: PathBuf,

    /// Filter pattern (supports * wildcards, case-sensitive)
    #[arg(long)]
    filter: Option<String>,

    /// Non-interactive mode: print a summary instead of launching the TUI
    #[arg(long, action = ArgAction::SetTrue)]
    no_ui: bool,

    /// Emit JSON of (optionally filtered) functions and exit
    #[arg(long, action = ArgAction::SetTrue)]
    json: bool,

    /// Limit number of rows printed in --no-ui summary (ignored for --json)
    #[arg(long, default_value_t = 20)]
    top: usize,

    /// Show raw names instead of best (demangled/export) names
    #[arg(long, action = ArgAction::SetTrue)]
    raw_names: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let module =
        parse_wasm(&cli.wasm_path).with_context(|| "Failed to parse the provided wasm file")?;

    // Build call graph once for TUI graph mode
    let wasm_bytes = std::fs::read(&cli.wasm_path)
        .with_context(|| format!("Failed to read file {}", cli.wasm_path.display()))?;
    // DWARF/addr2line context is initialized lazily on first mapping; no early call here to avoid private API exposure
    let call_graph = build_call_graph(&wasm_bytes).unwrap_or_default();

    if cli.json || cli.no_ui {
        run_non_interactive(&cli, &module)?;
        return Ok(());
    }

    run_tui(&cli, &module, call_graph, wasm_bytes)
}

fn run_non_interactive(cli: &Cli, module: &WasmModuleInfo) -> Result<()> {
    if cli.json {
        let items = collect_output_items(module, cli.filter.as_deref(), cli.raw_names);
        let out = JsonOutput {
            wasm_path: cli.wasm_path.to_string_lossy().into_owned(),
            total_code_size: module.total_code_size,
            imported_functions: module.imported_functions,
            defined_functions: module.defined_functions,
            functions: items,
        };
        let s = serde_json::to_string_pretty(&out)?;
        println!("{s}");
        return Ok(());
    }

    // Summary mode — compile glob once per filter for speed

    let mut matches: Vec<usize> = if let Some(p) = cli.filter.as_deref() {
        let pat = if p.contains('*') {
            p.to_string()
        } else {
            format!("*{}*", p)
        };
        if let Ok(glob) = GlobBuilder::new(&pat)
            .case_insensitive(true)
            .backslash_escape(false)
            .build()
        {
            let matcher = glob.compile_matcher();
            let mut v = Vec::with_capacity(module.defined_functions as usize);
            for (i, f) in module.functions.iter().enumerate() {
                let mut matched = false;
                if let Some(ref d) = f.demangled_name {
                    if matcher.is_match(d) {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(ref r) = f.raw_name {
                        if matcher.is_match(r) {
                            matched = true;
                        }
                    }
                }
                if !matched {
                    for e in &f.export_names {
                        if matcher.is_match(e) {
                            matched = true;
                            break;
                        }
                    }
                }
                if !matched
                    && f.demangled_name.is_none()
                    && f.raw_name.is_none()
                    && f.export_names.is_empty()
                {
                    let tmp = format!("func[{}]", f.index);
                    if matcher.is_match(&tmp) {
                        matched = true;
                    }
                }
                if matched {
                    v.push(i);
                }
            }
            v
        } else {
            Vec::new()
        }
    } else {
        (0..module.functions.len()).collect()
    };
    matches.sort_by_key(|i| std::cmp::Reverse(module.functions[*i].code_size));
    if matches.len() > cli.top {
        matches.truncate(cli.top);
    }

    println!(
        "wasm-poke summary: {}\nimports: {}  defined: {}  total code size: {}\n",
        cli.wasm_path.to_string_lossy(),
        module.imported_functions,
        module.defined_functions,
        module.total_code_size
    );

    println!(
        "{:>4}  {:>7}  {:>10}  {:>6}  {}",
        "Rank", "%", "Size", "Index", "Name"
    );
    println!("{}", "-".repeat(4 + 2 + 7 + 2 + 10 + 2 + 6 + 2 + 32));
    for (rank, idx) in matches.iter().enumerate() {
        let f = &module.functions[*idx];
        let pct = module.percentage(f);
        let name = display_name(f, cli.raw_names);
        println!(
            "{:>4}  {:>6.2}%  {:>10}  {:>6}  {}",
            rank + 1,
            pct,
            f.code_size,
            f.index,
            name
        );
    }

    Ok(())
}

struct App {
    wasm_path: String,
    module: WasmModuleInfo,

    indices: Vec<usize>,

    selected: usize,

    filter: String,

    in_search: bool,

    raw_names: bool,

    last_update: Instant,

    // cached wasm bytes for inspect mode
    wasm_bytes: Vec<u8>,

    // graph mode state
    graph_mode: bool,

    call_graph: CallGraph,

    graph_root: Option<u32>,

    expanded: std::collections::HashSet<Vec<(u32, usize)>>,

    tree_selected: usize,

    // inspect mode state
    inspect_mode: bool,

    // WAT lines and cursor for inspect
    wat_lines: Vec<WatLine>,

    wat_cursor: usize,

    // scroll offset for WAT pane (in lines)
    wat_scroll: u16,

    // scroll offset for Source pane (in lines)
    source_scroll: u16,

    // Cache for source spans (function index -> list of source spans)
    source_span_cache: std::collections::HashMap<u32, Vec<wasm_poke::SourceSpan>>,
    // Cache for source file content (filename -> content)
    source_file_cache: std::collections::HashMap<String, String>,
    // Manual override for source file view (filename)
    manual_source_file: Option<String>,

    // precomputed name map (global index -> best name)
    name_map: std::collections::HashMap<u32, String>,

    // caches for inspect mode data
    inspect_cache: std::collections::HashMap<u32, Vec<WatLine>>,

    // Help popup state
    help_popup: Option<String>,

    // Scroll offset for the graph tree view
    tree_scroll: usize,

    // Scroll offset for the main list view
    main_scroll: usize,

    // State for the source file list
    source_list_state: ListState,
}

impl App {
    fn new(
        path: String,
        module: WasmModuleInfo,
        filter: Option<String>,
        raw_names: bool,
        wasm_bytes: Vec<u8>,
        call_graph: CallGraph,
    ) -> Self {
        let mut app = Self {
            wasm_path: path,
            indices: Vec::new(),
            selected: 0,
            filter: filter.unwrap_or_default(),
            in_search: false,
            module: module.clone(),
            raw_names,
            last_update: Instant::now(),
            // cache wasm bytes
            wasm_bytes,
            graph_mode: false,
            call_graph,
            graph_root: None,
            expanded: std::collections::HashSet::new(),
            tree_selected: 0,
            inspect_mode: false,
            wat_lines: Vec::new(),
            wat_cursor: 0,
            wat_scroll: 0,
            source_scroll: 0,
            source_span_cache: std::collections::HashMap::new(),
            source_file_cache: std::collections::HashMap::new(),
            manual_source_file: None,
            name_map: module
                .functions
                .iter()
                .map(|f| (f.index, f.best_name()))
                .collect(),

            // initialize caches
            inspect_cache: std::collections::HashMap::new(),


            help_popup: None,
            tree_scroll: 0,
            main_scroll: 0,
            source_list_state: ListState::default(),
        };

        app.refresh_indices();
        app
    }

    fn refresh_indices(&mut self) {
        // Build sorted-by-size indices each refresh (fast and simple, avoids stale caches)
        let mut all: Vec<usize> = (0..self.module.functions.len()).collect();
        all.sort_by_key(|i| std::cmp::Reverse(self.module.functions[*i].code_size));

        if self.filter.is_empty() {
            self.indices = all;
        } else {
            let terms: Vec<&str> = self.filter.split_whitespace().collect();
            
            // compile matchers for each term
            let matchers: Vec<_> = terms
                .iter()
                .filter_map(|term| {
                    let normalized = if term.contains('*') {
                        term.to_string()
                    } else {
                        format!("*{}*", term)
                    };
                    GlobBuilder::new(&normalized)
                        .case_insensitive(true)
                        .backslash_escape(false)
                        .build()
                        .ok()
                        .map(|g| g.compile_matcher())
                })
                .collect();

            if matchers.len() != terms.len() {
                // invalid pattern -> no matches
                self.indices.clear();
                if self.selected >= self.indices.len() {
                    self.selected = self.indices.len().saturating_sub(1);
                }
                return;
            }

            // filter against the freshly sorted indices
            self.indices.clear();

            for &i in &all {
                let f = &self.module.functions[i];
                let mut all_terms_matched = true;

                for matcher in &matchers {
                    let mut term_matched = false;

                    if let Some(ref d) = f.demangled_name {
                        if matcher.is_match(d) {
                            term_matched = true;
                        }
                    }

                    if !term_matched {
                        if let Some(ref r) = f.raw_name {
                            if matcher.is_match(r) {
                                term_matched = true;
                            }
                        }
                    }

                    if !term_matched {
                        for e in &f.export_names {
                            if matcher.is_match(e) {
                                term_matched = true;
                                break;
                            }
                        }
                    }

                    if !term_matched
                        && f.demangled_name.is_none()
                        && f.raw_name.is_none()
                        && f.export_names.is_empty()
                    {
                        let tmp = format!("func[{}]", f.index);
                        if matcher.is_match(&tmp) {
                            term_matched = true;
                        }
                    }

                    if !term_matched {
                        all_terms_matched = false;
                        break;
                    }
                }

                if all_terms_matched {
                    self.indices.push(i);
                }
            }
        }

        if self.selected >= self.indices.len() {
            self.selected = self.indices.len().saturating_sub(1);
        }
    }

    fn selected_function(&self) -> Option<&FunctionInfo> {
        self.indices
            .get(self.selected)
            .and_then(|i| self.module.functions.get(*i))
    }

    fn ensure_inspect_assets(&mut self, func_index: u32) {
        // Cache source spans once
        if !self.source_span_cache.contains_key(&func_index) {
            let spans = wasm_poke::function_source_span(&self.wasm_bytes, func_index);
            if !spans.is_empty() {
                self.source_span_cache.insert(func_index, spans);
            }
        }

        // Cache disassembled WAT lines once
        if !self.inspect_cache.contains_key(&func_index) {
            let mut lines =
                disassemble_function_wat_lines(&self.wasm_bytes, func_index).unwrap_or_default();
            
            // Pre-compute source mappings for all lines
            for line in &mut lines {
                line.src = wasm_poke::map_instr_to_source_fast(
                    &self.module,
                    &self.wasm_bytes,
                    func_index,
                    line.offset,
                );
            }

            self.inspect_cache.insert(func_index, lines);
        }

        // Cache source file contents for all spans
        if let Some(spans) = self.source_span_cache.get(&func_index) {
            for span in spans {
                if !self.source_file_cache.contains_key(&span.file) {
                    if let Ok(src) = std::fs::read_to_string(&span.file) {
                        self.source_file_cache.insert(span.file.clone(), src);
                    }
                }
            }
        }
    }

    fn current_inspect_function(&self) -> Option<u32> {
        if self.graph_mode {
            let (_, rows) = self.compute_tree_view(Some(self.tree_selected..self.tree_selected + 1));
            rows.first().map(|r| r.index)
        } else {
            self.selected_function().map(|f| f.index)
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> bool {
        // Returns true to request exit
        if key.kind != KeyEventKind::Press {
            return false;
        }

        if self.inspect_mode {
            match key.code {
                KeyCode::Char('[') => {
                    if let Some(idx) = self.current_inspect_function() {
                        if let Some(spans) = self.source_span_cache.get(&idx) {
                            if !spans.is_empty() {
                                let current = self.manual_source_file.as_ref().or_else(|| spans.first().map(|s| &s.file));
                                let pos = spans.iter().position(|s| Some(&s.file) == current).unwrap_or(0);
                                let new_pos = if pos == 0 { spans.len() - 1 } else { pos - 1 };
                                self.manual_source_file = Some(spans[new_pos].file.clone());
                            }
                        }
                    }
                    return false;
                }
                KeyCode::Char(']') => {
                    if let Some(idx) = self.current_inspect_function() {
                        if let Some(spans) = self.source_span_cache.get(&idx) {
                            if !spans.is_empty() {
                                let current = self.manual_source_file.as_ref().or_else(|| spans.first().map(|s| &s.file));
                                let pos = spans.iter().position(|s| Some(&s.file) == current).unwrap_or(0);
                                let new_pos = (pos + 1) % spans.len();
                                self.manual_source_file = Some(spans[new_pos].file.clone());
                            }
                        }
                    }
                    return false;
                }
                _ => {}
            }
        }

        if self.in_search {
            match key.code {
                KeyCode::Esc => {
                    self.in_search = false;
                }
                KeyCode::Enter => {
                    self.in_search = false;
                    self.refresh_indices();
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.refresh_indices();
                }
                KeyCode::Char(c) => {
                    // if user presses Ctrl+u, clear
                    if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'u' || c == 'U') {
                        self.filter.clear();
                        self.refresh_indices();
                    } else {
                        self.filter.push(c);
                        self.refresh_indices();
                    }
                }
                _ => {}
            }
            return false;
        }

        // Help popup handling
        if self.help_popup.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') => {
                    self.help_popup = None;
                }
                _ => {}
            }
            return false;
        }

        // Graph mode handling
        if self.graph_mode {
            match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('g') => {
                    // toggle off graph mode
                    self.graph_mode = false;
                }
                KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Up => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_sub(1);
                    } else if self.tree_selected > 0 {
                        self.tree_selected -= 1;
                    }
                }
                KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Down => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_add(1);
                    } else {
                        let (total, _) = self.compute_tree_view(Some(0..0));
                        let max = total.saturating_sub(1);
                        self.tree_selected = (self.tree_selected + 1).min(max);
                    }
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                    // collapse current node if expanded
                    // Get current row
                    let (_, rows) = self.compute_tree_view(Some(self.tree_selected..self.tree_selected + 1));
                    if let Some(row) = rows.first() {
                        if self.expanded.remove(&row.path) {
                            // collapsed current node
                        } else if self.tree_selected > 0 {
                            // if not expanded, try to move selection to parent (upwards)
                            self.tree_selected -= 1;
                        }
                    }
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') | KeyCode::Char('L') => {
                    let (_, rows) = self.compute_tree_view(Some(self.tree_selected..self.tree_selected + 1));
                    if let Some(row) = rows.first() {
                        // expand if it has children
                        if self
                            .call_graph
                            .edges
                            .get(&row.index)
                            .map(|v| !v.is_empty())
                            .unwrap_or(false)
                        {
                            self.expanded.insert(row.path.clone());
                        }
                    }
                }
                KeyCode::Home => {
                    self.tree_selected = 0;
                    self.tree_scroll = 0;
                }
                KeyCode::End => {
                    let (total, _) = self.compute_tree_view(Some(0..0));
                    if total > 0 {
                        self.tree_selected = total - 1;
                    }
                }
                KeyCode::PageUp | KeyCode::Char('u') => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_sub(10);
                    } else {
                        let step = 10usize;
                        if self.tree_selected >= step {
                            self.tree_selected -= step;
                        } else {
                            self.tree_selected = 0;
                        }
                    }
                }
                KeyCode::PageDown | KeyCode::Char('d') => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_add(10);
                    } else {
                        let (total, _) = self.compute_tree_view(Some(0..0));
                        let step = 10usize;
                        self.tree_selected =
                            (self.tree_selected + step).min(total.saturating_sub(1));
                    }
                }
                KeyCode::Char('/') => {
                    self.in_search = true;
                }
                KeyCode::Char('r') => {
                    self.raw_names = !self.raw_names;
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    // Toggle inspect mode in graph view
                    if self.inspect_mode {
                        self.inspect_mode = false;
                    } else {
                        self.inspect_mode = true;
                        self.wat_scroll = 0;
                        self.source_scroll = 0;
                        self.wat_cursor = 0;
                        self.wat_cursor = 0;
                        let (_, rows) = self.compute_tree_view(Some(self.tree_selected..self.tree_selected + 1));
                        if let Some(row) = rows.first() {
                            let idx = row.index;
                            // Ensure inspect assets (WAT lines, source span, source file) are cached
                            self.ensure_inspect_assets(idx);
                            self.wat_lines =
                                self.inspect_cache.get(&idx).cloned().unwrap_or_default();
                        } else {
                            self.wat_lines.clear();
                        }
                    }
                }
                KeyCode::Char('?') => {
                    if self.inspect_mode {
                        if let Some(line) = self.wat_lines.get(self.wat_cursor) {
                            // Extract mnemonic: first token after trimming whitespace
                            let trimmed = line.text.trim();
                            if let Some(mnemonic) = trimmed.split_whitespace().next() {
                                if let Some(help) = wasm_poke::help::get_instruction_help(mnemonic) {
                                    self.help_popup = Some(format!("{} — {}", mnemonic, help));
                                } else {
                                    self.help_popup = Some(format!("No help available for '{}'", mnemonic));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('g') => {
                if self.inspect_mode {
                    // "Goto" definition if on a call instruction
                    if let Some(line) = self.wat_lines.get(self.wat_cursor) {
                        let trimmed = line.text.trim();
                        // Check for "call <index>"
                        // The format from lib.rs is "call {function_index}"
                        if let Some(rest) = trimmed.strip_prefix("call ") {
                            // The rest should be the index (and maybe comments)
                            let token = rest.split_whitespace().next().unwrap_or("");
                            if let Ok(target_idx) = token.parse::<u32>() {
                                // We found a target!
                                // 1. Ensure we are in graph mode (conceptually)
                                if !self.graph_mode {
                                    // If we were in list mode, we need to "start" graph mode
                                    // effectively rooting at the *current* function (caller),
                                    // then expanding to the callee.
                                    if let Some(current_func) = self.selected_function() {
                                        self.graph_root = Some(current_func.index);
                                        self.expanded.clear();
                                        self.expanded.clear();
                                        self.tree_selected = 0;
                                        self.tree_scroll = 0;
                                        self.graph_mode = true;
                                    }
                                }

                                // 2. Now we are in graph mode. We need to find the row for `target_idx`
                                //    that is a child of the currently selected tree node.
                                
                                // First, expand the current node so children are visible
                                let (_, rows) = self.compute_tree_view(Some(self.tree_selected..self.tree_selected + 1));
                                if let Some(current_row) = rows.first() {
                                    self.expanded.insert(current_row.path.clone());
                                    
                                    // Now find the child
                                    if let Some(child_row_idx) = self.find_visible_child_row(&current_row.path, target_idx) {
                                        self.tree_selected = child_row_idx;
                                        
                                        // 3. Update inspect view to the new function
                                        self.wat_scroll = 0;
                                        self.source_scroll = 0;
                                        self.wat_cursor = 0;
                                        self.ensure_inspect_assets(target_idx);
                                        self.wat_lines = self
                                            .inspect_cache
                                            .get(&target_idx)
                                            .cloned()
                                            .unwrap_or_default();
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // enter graph mode with current selection as root (original behavior)
                    if let Some(f) = self.selected_function() {
                        self.graph_root = Some(f.index);
                        self.expanded.clear();
                        self.expanded.clear();
                        self.tree_selected = 0;
                        self.tree_scroll = 0;
                        self.graph_mode = true;
                        
                        // Clear filter so graph view isn't restricted
                        self.filter.clear();
                        self.refresh_indices();
                    }
                }
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                // Toggle inspect mode in list view
                if self.inspect_mode {
                    self.inspect_mode = false;
                } else if let Some(f) = self.selected_function() {
                    let func_index = f.index;
                    self.inspect_mode = true;
                    self.wat_scroll = 0;
                    self.source_scroll = 0;
                    self.wat_cursor = 0;
                    // Ensure inspect assets (WAT lines, source span, source file) are cached
                    self.ensure_inspect_assets(func_index);
                    self.wat_lines = self
                        .inspect_cache
                        .get(&func_index)
                        .cloned()
                        .unwrap_or_default();
                }
            }
            KeyCode::Char('/') => {
                self.in_search = true;
            }
            KeyCode::Char('c') => {
                self.filter.clear();
                self.refresh_indices();
            }
            KeyCode::Char('r') => {
                self.raw_names = !self.raw_names;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.inspect_mode {
                    self.wat_cursor = self.wat_cursor.saturating_sub(1);
                } else if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.inspect_mode {
                    self.wat_cursor = self.wat_cursor.saturating_add(1);
                } else if self.selected + 1 < self.indices.len() {
                    self.selected += 1;
                }
            }
            KeyCode::PageUp => {
                if self.inspect_mode {
                    self.wat_cursor = self.wat_cursor.saturating_sub(10);
                } else {
                    let step = 10usize;
                    if self.selected >= step {
                        self.selected -= step;
                    } else {
                        self.selected = 0;
                    }
                }
            }
            KeyCode::PageDown => {
                if self.inspect_mode {
                    self.wat_cursor = self.wat_cursor.saturating_add(10);
                } else {
                    let step = 10usize;
                    self.selected =
                        (self.selected + step).min(self.indices.len().saturating_sub(1));
                }
            }
            KeyCode::Home => {
                self.selected = 0;
                self.main_scroll = 0;
            }
            KeyCode::End => {
                if !self.indices.is_empty() {
                    self.selected = self.indices.len() - 1;
                }
            }
            KeyCode::Char('?') => {
                if self.inspect_mode {
                    if let Some(line) = self.wat_lines.get(self.wat_cursor) {
                        // Extract mnemonic: first token after trimming whitespace
                        let trimmed = line.text.trim();
                        if let Some(mnemonic) = trimmed.split_whitespace().next() {
                            if let Some(help) = wasm_poke::help::get_instruction_help(mnemonic) {
                                self.help_popup = Some(format!("{} — {}", mnemonic, help));
                            } else {
                                self.help_popup = Some(format!("No help available for '{}'", mnemonic));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        false
    }

    // Compute visible tree rows based on root and expanded nodes.
    // Returns (total_count, visible_rows)
    // If window is Some, only allocates and returns rows within that range.
    // If window is None, returns all rows (careful with large graphs!).
    fn compute_tree_view(&self, window: Option<std::ops::Range<usize>>) -> (usize, Vec<TreeRow>) {
        let root = if let Some(r) = self.graph_root {
            r
        } else {
            return (0, Vec::new());
        };

        // If filtering is active, we fall back to generating the filtered list (expensive but necessary for correctness)
        // We could optimize this further, but the OOM is likely from the raw view.
        if !self.filter.is_empty() {
            return self.compute_tree_view_filtered(root, window);
        }

        let mut rows: Vec<TreeRow> = Vec::new();
        let mut count = 0;

        let mut path: Vec<(u32, usize)> = Vec::new();
        let mut stack: Vec<(u32, usize, usize)> = Vec::new(); // (node, depth, child_index)
        stack.push((root, 0, 0));
        path.push((root, 0));

        while let Some((node, depth, mut child_i)) = stack.pop() {
            // visit this node (when first popping with child_i == 0)
            if child_i == 0 {
                let include = if let Some(ref w) = window {
                    count >= w.start && count < w.end
                } else {
                    true
                };

                if include {
                    let has_indirect = *self.call_graph.has_indirect.get(&node).unwrap_or(&false);
                    rows.push(TreeRow {
                        depth,
                        index: node,
                        is_cycle: false, // We don't track cycle for the root/downward pass easily here without path check, but let's rely on the child check
                        has_indirect,
                        path: path.clone(),
                    });
                }
                count += 1;
            }

            // expand children only if expanded contains node path
            if self.expanded.contains(&path) {
                if let Some(children) = self.call_graph.edges.get(&node) {
                    if child_i < children.len() {
                        let current_child_idx = child_i;
                        let child = children[child_i];
                        child_i += 1;
                        // put current back
                        stack.push((node, depth, child_i));
                        
                        if path.iter().any(|(n, _)| *n == child) {
                            // cycle marker
                            let include = if let Some(ref w) = window {
                                count >= w.start && count < w.end
                            } else {
                                true
                            };
                            if include {
                                let has_indirect = *self.call_graph.has_indirect.get(&child).unwrap_or(&false);
                                // temporarily push child to path for the row
                                path.push((child, current_child_idx));
                                rows.push(TreeRow {
                                    depth: depth + 1,
                                    index: child,
                                    is_cycle: true,
                                    has_indirect,
                                    path: path.clone(),
                                });
                                path.pop();
                            }
                            count += 1;
                        } else {
                            // descend
                            path.push((child, current_child_idx));
                            stack.push((child, depth + 1, 0));
                        }
                    } else {
                        // done with children
                        let _ = path.pop();
                    }
                } else {
                    // no children
                    let _ = path.pop();
                }
            } else {
                // collapsed
                let _ = path.pop();
            }
        }

        (count, rows)
    }

    // Fallback for filtered view: generates all candidates then filters.
    // This is still memory intensive but required for correct filtering logic.
    fn compute_tree_view_filtered(&self, root: u32, window: Option<std::ops::Range<usize>>) -> (usize, Vec<TreeRow>) {
        // 1. Generate ALL rows (virtualized walker logic but collecting all)
        // We can't use the window yet because we don't know which ones match.
        let mut all_rows = Vec::new();
        
        let mut path: Vec<(u32, usize)> = Vec::new();
        let mut stack: Vec<(u32, usize, usize)> = Vec::new();
        stack.push((root, 0, 0));
        path.push((root, 0));

        // Helper to push
        let mut push_row = |idx: u32, depth: usize, is_cycle: bool, p: &[(u32, usize)]| {
             let has_indirect = *self.call_graph.has_indirect.get(&idx).unwrap_or(&false);
             all_rows.push(TreeRow {
                 depth,
                 index: idx,
                 is_cycle,
                 has_indirect,
                 path: p.to_vec(),
             });
        };

        while let Some((node, depth, mut child_i)) = stack.pop() {
            if child_i == 0 {
                push_row(node, depth, false, &path);
            }

            if self.expanded.contains(&path) {
                if let Some(children) = self.call_graph.edges.get(&node) {
                    if child_i < children.len() {
                        let current_child_idx = child_i;
                        let child = children[child_i];
                        child_i += 1;
                        stack.push((node, depth, child_i));
                        if path.iter().any(|(n, _)| *n == child) {
                            path.push((child, current_child_idx));
                            push_row(child, depth + 1, true, &path);
                            path.pop();
                        } else {
                            path.push((child, current_child_idx));
                            stack.push((child, depth + 1, 0));
                        }
                    } else {
                        let _ = path.pop();
                    }
                } else {
                    let _ = path.pop();
                }
            } else {
                let _ = path.pop();
            }
        }

        // 2. Apply filter
        let pat = if self.filter.contains('*') {
            self.filter.clone()
        } else {
            format!("*{}*", self.filter)
        };
        
        let mut match_flags: Vec<bool> = Vec::with_capacity(all_rows.len());
        for r in &all_rows {
            let is_match = if let Some(f) = self.module.functions.iter().find(|f| f.index == r.index) {
                wasm_poke::function_matches(f, &pat)
            } else {
                false
            };
            match_flags.push(is_match);
        }

        let mut keep: Vec<bool> = vec![false; all_rows.len()];
        let mut stack_idx: Vec<usize> = Vec::new();
        for (i, r) in all_rows.iter().enumerate() {
            while stack_idx.len() > r.depth {
                stack_idx.pop();
            }
            if match_flags[i] {
                keep[i] = true;
                for &anc in &stack_idx {
                    keep[anc] = true;
                }
            }
            stack_idx.push(i);
        }

        let filtered_rows: Vec<TreeRow> = all_rows
            .into_iter()
            .zip(keep.into_iter())
            .filter_map(|(r, k)| if k { Some(r) } else { None })
            .collect();

        let total = filtered_rows.len();
        let visible = if let Some(w) = window {
            filtered_rows.into_iter().skip(w.start).take(w.end - w.start).collect()
        } else {
            filtered_rows
        };

        (total, visible)
    }

    // Helper to find the row index of a specific child of the current selection.
    // Used for 'goto' functionality.
    fn find_visible_child_row(&self, parent_path: &[(u32, usize)], child_index: u32) -> Option<usize> {
        let root = self.graph_root?;
        
        // We walk the tree until we find the node that matches.
        // This is essentially a search.
        
        let mut count = 0;
        let mut path: Vec<(u32, usize)> = Vec::new();
        let mut stack: Vec<(u32, usize, usize)> = Vec::new();
        stack.push((root, 0, 0));
        path.push((root, 0));

        while let Some((node, depth, mut child_i)) = stack.pop() {
            if child_i == 0 {
                // Check if this is the node we are looking for
                // It must match the child_index AND its parent path must match parent_path
                if node == child_index && path.len() == parent_path.len() + 1 && path.starts_with(parent_path) {
                    return Some(count);
                }
                count += 1;
            }

            if self.expanded.contains(&path) {
                if let Some(children) = self.call_graph.edges.get(&node) {
                    if child_i < children.len() {
                        let current_child_idx = child_i;
                        let child = children[child_i];
                        child_i += 1;
                        stack.push((node, depth, child_i));
                        if path.iter().any(|(n, _)| *n == child) {
                            // cycle
                            if child == child_index && path.len() == parent_path.len() && path.starts_with(parent_path) {
                                // cycle child match?
                                if path.len() == parent_path.len() {
                                     return Some(count);
                                }
                            }
                            count += 1;
                        } else {
                            path.push((child, current_child_idx));
                            stack.push((child, depth + 1, 0));
                        }
                    } else {
                        let _ = path.pop();
                    }
                } else {
                    let _ = path.pop();
                }
            } else {
                let _ = path.pop();
            }
        }
        None
    }
}

fn run_tui(
    cli: &Cli,
    module: &WasmModuleInfo,
    call_graph: CallGraph,
    wasm_bytes: Vec<u8>,
) -> Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(
        cli.wasm_path.to_string_lossy().into_owned(),
        module.clone(),
        cli.filter.clone(),
        cli.raw_names,
        wasm_bytes,
        call_graph,
    );

    let res = run_ui_loop(&mut terminal, &mut app);

    // Restore
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    res
}

fn run_ui_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        // Poll input with a small timeout to allow periodic UI updates if needed
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.on_key(key) {
                        break;
                    }
                }
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        // Throttle any background refresh if we add one later
        let _ = app.last_update.elapsed();
    }
    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(f.area());

    draw_header(f, chunks[0], app);
    draw_table(f, chunks[1], app);
    draw_footer(f, chunks[2], app);

    if let Some(text) = &app.help_popup {
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(60) / 2,
            y: area.height.saturating_sub(10) / 2,
            width: 60.min(area.width),
            height: 10.min(area.height),
        };
        f.render_widget(ratatui::widgets::Clear, popup_area);
        let p = Paragraph::new(text.as_str())
            .block(Block::default().borders(Borders::ALL).title(" Instruction Help "))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White).bg(Color::Blue));
        f.render_widget(p, popup_area);
    }
}

fn draw_header(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let title = format!(
        "wasm-poke — {} | imports: {}  defined: {}  total code size: {}",
        app.wasm_path,
        app.module.imported_functions,
        app.module.defined_functions,
        app.module.total_code_size
    );

    let filter_line = if app.in_search {
        format!(
            "Filter (use * wildcards, Enter apply, Esc cancel): {}",
            app.filter
        )
    } else if app.filter.is_empty() {
        "Filter: (none) — press '/' to edit, 'c' to clear".to_string()
    } else {
        format!("Filter: {}", app.filter)
    };

    // live stats for current filter: matches, total bytes, and percent of total code size
    let matched_count = app.indices.len();
    let total_funcs = app.module.functions.len();
    let matched_bytes: u64 = app
        .indices
        .iter()
        .map(|&i| app.module.functions[i].code_size as u64)
        .sum();
    let pct = if app.module.total_code_size == 0 {
        0.0
    } else {
        (matched_bytes as f64) * 100.0 / (app.module.total_code_size as f64)
    };
    let stats_line = format!(
        "Matches: {}/{}  |  Filtered bytes: {}  ({:.2}%)",
        matched_count, total_funcs, matched_bytes, pct
    );

    let text = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            filter_line,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(stats_line, Style::default().fg(Color::Green))),
    ];

    let block = Block::default().borders(Borders::ALL).title(" Overview ");
    let p = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

struct WatRenderer;

impl WatRenderer {
    fn render_window(
        lines: &[WatLine],
        cursor: usize,
        top: usize,
        vis: usize,
        name_map: &std::collections::HashMap<u32, String>,
        highlight_target: Option<&wasm_poke::SourceLocation>,
    ) -> Vec<Line<'static>> {
        let mut output = Vec::new();
        for (i, wl) in lines.iter().enumerate().skip(top).take(vis) {
            // Use a space to preserve alignment where the caret used to be, 
            // or just indent slightly.
            let mut rendered = format!("  {}", wl.text);
            let t = wl.text.trim_start();
            if let Some(rest) = t.strip_prefix("call ") {
                if let Some((num, _)) = rest.split_once(' ') {
                    if let Ok(idx) = num.parse::<u32>() {
                        if let Some(name) = name_map.get(&idx) {
                            rendered.push_str("  ;; ");
                            rendered.push_str(name);
                        }
                    }
                } else if let Ok(idx) = rest.parse::<u32>() {
                    if let Some(name) = name_map.get(&idx) {
                        rendered.push_str("  ;; ");
                        rendered.push_str(name);
                    }
                }
            }
            
            let mut line = Line::from(rendered);
            if i == cursor {
                line = line.patch_style(Style::default().bg(Color::DarkGray));
            } else if let Some(target) = highlight_target {
                // Secondary highlight if source matches
                if let Some(src) = &wl.src {
                    if src.file == target.file && src.line == target.line {
                        line = line.patch_style(Style::default().bg(Color::Rgb(50, 50, 50)));
                    }
                }
            }
            output.push(line);
        }
        output
    }
}

#[derive(Debug, Clone)]
struct TreeRow {
    depth: usize,
    index: u32,
    is_cycle: bool,
    has_indirect: bool,
    path: Vec<(u32, usize)>,
}

fn draw_table(f: &mut ratatui::Frame<'_>, area: Rect, app: &mut App) {
    // Inspect mode rendering (side-by-side hex and WAT)
    if app.inspect_mode {
        // Determine current function index based on current mode/selection
        let current_index = app.current_inspect_function();

        let current_index = if let Some(ix) = current_index {
            ix
        } else {
            // Nothing selected
            let p = Paragraph::new("No function selected for inspect")
                .block(Block::default().borders(Borders::ALL).title(" Inspect "));
            f.render_widget(p, area);
            return;
        };

        // Slice function body bytes from cached wasm
        let body_bytes =
            wasm_poke::function_body_bytes(&app.module, &app.wasm_bytes, current_index)
                .unwrap_or(&[]);

        // Use cached WAT lines (populated on entering inspect)
        let wat_lines: Vec<WatLine> = if app.wat_lines.is_empty() {
            disassemble_function_wat_lines(&app.wasm_bytes, current_index).unwrap_or_default()
        } else {
            app.wat_lines.clone()
        };
        let total_wat_lines = wat_lines.len();
        let cursor = app.wat_cursor.min(total_wat_lines.saturating_sub(1));

        // Symbol Name Header
        let full_name = if current_index < app.module.imported_functions {
            // It's an import
            app.name_map
                .get(&current_index)
                .cloned()
                .unwrap_or_else(|| format!("import[{}]", current_index))
        } else {
            // It's a defined function
            let func_idx = (current_index - app.module.imported_functions) as usize;
            if let Some(func) = app.module.functions.get(func_idx) {
                display_name(func, app.raw_names)
            } else {
                format!("func[{}] (missing info)", current_index)
            }
        };

        // Calculate required height for the name
        let available_width = area.width.saturating_sub(2);
        let name_len = full_name.len() as u16;
        // integer ceil div
        let name_lines = (name_len + available_width.saturating_sub(1)) / available_width.max(1);
        // Clamp to a reasonable maximum to avoid taking up the whole screen if something goes wrong,
        // but 10 lines is plenty for a name.
        let header_height = (name_lines + 2).min(10);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Min(0),
            ])
            .split(area);

        let header_area = chunks[0];
        let content_area = chunks[1];

        let header_paragraph = Paragraph::new(full_name)
            .block(Block::default().borders(Borders::ALL).title(" Symbol "))
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true });
        f.render_widget(header_paragraph, header_area);

        // Split horizontally into three panes: Hex | WAT | Source
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(79), // Fixed width for Hex (77 content + 2 border)
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(content_area);

        // HEX pane: window around selected byte and color that byte
        let visible_hex_lines: usize = cols[0].height.saturating_sub(2) as usize;
        let bytes_per_line: usize = 16;
        let selected_offset = wat_lines.get(cursor).map(|w| w.offset).unwrap_or(0usize);
        let selected_row = selected_offset / bytes_per_line;
        let selected_col = selected_offset % bytes_per_line;
        let total_hex_lines = (body_bytes.len() + bytes_per_line - 1) / bytes_per_line;
        let desired_hex_top = selected_row.saturating_sub(visible_hex_lines / 2);
        let max_hex_top = total_hex_lines.saturating_sub(visible_hex_lines);
        let hex_top = desired_hex_top.min(max_hex_top);

        let mut hex_lines: Vec<Line> = Vec::with_capacity(visible_hex_lines);
        for r in hex_top..(hex_top + visible_hex_lines).min(total_hex_lines) {
            let base = r * bytes_per_line;
            let end = (base + bytes_per_line).min(body_bytes.len());
            let mut spans: Vec<Span> = Vec::with_capacity(1 + bytes_per_line + 2 + (end - base));
            spans.push(Span::raw(format!("{:08x}: ", base)));

            for c in 0..bytes_per_line {
                if base + c < end {
                    let b = body_bytes[base + c];
                    let s = format!("{:02x} ", b);
                    if r == selected_row && c == selected_col {
                        spans.push(Span::styled(
                            s,
                            Style::default().fg(Color::Black).bg(Color::Yellow),
                        ));
                    } else {
                        spans.push(Span::raw(s));
                    }
                } else {
                    spans.push(Span::raw("   "));
                }
            }

            // ASCII gutter
            spans.push(Span::raw(" |"));
            for i in base..end {
                let ch = body_bytes[i];
                let ch = if ch.is_ascii_graphic() || ch == b' ' {
                    ch as char
                } else {
                    '.'
                };
                spans.push(Span::raw(ch.to_string()));
            }
            spans.push(Span::raw("|"));
            hex_lines.push(Line::from(spans));
        }
        let hex_widget =
            Paragraph::new(hex_lines).block(Block::default().borders(Borders::ALL).title(" Hex "));

        // WAT pane: center cursor using our windowed renderer
        let visible_wat_lines: usize = cols[1].height.saturating_sub(2) as usize;
        let desired_wat_top = cursor.saturating_sub(visible_wat_lines / 2);
        let max_wat_top = total_wat_lines.saturating_sub(visible_wat_lines);
        let wat_top = desired_wat_top.min(max_wat_top);

        let wat_title = if let Some(spans) = app.source_span_cache.get(&current_index) {
             if let Some(first) = spans.first() {
                 format!(" WAT — {}:{} ", first.file, first.start_line)
             } else {
                 " WAT ".to_string()
             }
        } else {
            " WAT ".to_string()
        };

        // Source pane split: File List (top) and Content (bottom)
        let source_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Title + 4 files + border
                Constraint::Min(0),
            ])
            .split(cols[2]);

        // Source pane logic
        let spans = app.source_span_cache.get(&current_index).map(|v| v.as_slice()).unwrap_or(&[]);

        // Determine active file
        // 1. Manual override
        // 2. Map from current instruction
        // 3. First in list (dominant)
        let mut active_file = app.manual_source_file.clone();
        let mut target_line = None;
        let mut target_src_loc: Option<wasm_poke::SourceLocation> = None;

        // Try to get mapping from current cursor line
        if let Some(wl) = wat_lines.get(cursor) {
            if let Some(loc) = &wl.src {
                target_src_loc = Some(loc.clone());
                if active_file.is_none() {
                    active_file = Some(loc.file.clone());
                }
                if Some(&loc.file) == active_file.as_ref() {
                    target_line = Some(loc.line);
                }
            }
        }

        if active_file.is_none() {
            active_file = spans.first().map(|s| s.file.clone());
        }

        // Fallback target line to start of file/span if still unknown
        if target_line.is_none() {
            if let Some(f) = &active_file {
                if let Some(s) = spans.iter().find(|s| &s.file == f) {
                    target_line = Some(s.start_line);
                }
            }
        }

        // Render File List
        let mut file_list_items = Vec::new();
        let mut active_idx = None;
        
        // Filter spans if we have a specific target from the instruction
        let visible_spans: Vec<&wasm_poke::SourceSpan> = if let Some(loc) = &target_src_loc {
            spans.iter().filter(|s| s.file == loc.file).collect()
        } else {
            spans.iter().collect()
        };

        if visible_spans.is_empty() {
            // Fallback to showing all if filtering resulted in nothing (shouldn't happen if logic is correct)
            // or if spans was empty to begin with.
            if spans.is_empty() {
                file_list_items.push(ListItem::new(Line::from("No source files found")));
            } else {
                 // If we filtered out everything (e.g. mapping points to file not in span list?), show all
                 for (i, s) in spans.iter().enumerate() {
                    let is_active = Some(&s.file) == active_file.as_ref();
                    if is_active {
                        active_idx = Some(i);
                    }
                    let style = if is_active {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    let prefix = if is_active { ">> " } else { "   " };
                    file_list_items.push(ListItem::new(Line::from(format!("{}{}", prefix, s.file)).style(style)));
                }
            }
        } else {
            for (i, s) in visible_spans.iter().enumerate() {
                let is_active = Some(&s.file) == active_file.as_ref();
                // Since we are filtering, the active file should be one of these, likely the only one.
                // But we still check to set the style.
                // Note: active_idx needs to match the index in the *displayed* list for the widget state?
                // Actually ListState index corresponds to the index in the items vector.
                if is_active {
                    active_idx = Some(i);
                }
                let style = if is_active {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                let prefix = if is_active { ">> " } else { "   " };
                file_list_items.push(ListItem::new(Line::from(format!("{}{}", prefix, s.file)).style(style)));
            }
        }
        
        // Update selection state for auto-scrolling
        if active_idx.is_some() {
            app.source_list_state.select(active_idx);
        } else {
            app.source_list_state.select(None);
        }

        let file_list = List::new(file_list_items)
            .block(Block::default().borders(Borders::ALL).title(" Source Files ([ / ]) "));

        // Render Content
        let mut source_lines: Vec<Line> = Vec::new();
        if let Some(f) = &active_file {
            if let Some(src) = app.source_file_cache.get(f) {
                let lines: Vec<&str> = src.lines().collect();
                let visible = source_chunks[1].height.saturating_sub(2) as usize;
                let target = target_line.unwrap_or(1) as usize;
                let half = visible / 2;
                let start = target.saturating_sub(1).saturating_sub(half);
                let end = (start + visible).min(lines.len());

                for i in start..end {
                    let ln = i + 1;
                    let content = format!("  {:5} | {}", ln, lines[i]);
                    let mut line = Line::from(content);
                    if Some(ln as u32) == target_line {
                        line = line.patch_style(Style::default().bg(Color::DarkGray));
                    }
                    source_lines.push(line);
                }
            } else {
                 source_lines.push(Line::from(format!("Could not read file: {}", f)));
            }
        } else {
            source_lines.push(Line::from("No source file selected."));
            source_lines.push(Line::from("Build with debug information (DWARF) to enable source pane."));
        }

        let source_content = Paragraph::new(source_lines)
            .block(Block::default().borders(Borders::ALL).title(format!(" Source: {} ", active_file.as_deref().unwrap_or("none"))));

        // Render panes
        f.render_widget(hex_widget, cols[0]);
        
        // Custom WAT rendering to highlight ranges
        // We need to pass the target_src_loc to the renderer or handle it here.
        // Since WatRenderer::render_window is simple, let's update it or inline the logic.
        // For now, let's update WatRenderer to accept an optional highlight target.
        let wat_text = WatRenderer::render_window(
            &wat_lines,
            cursor,
            wat_top,
            visible_wat_lines,
            &app.name_map,
            target_src_loc.as_ref(),
        );
        
        let wat_widget =
            Paragraph::new(wat_text).block(Block::default().borders(Borders::ALL).title(wat_title));

        f.render_widget(wat_widget, cols[1]);
        f.render_stateful_widget(file_list, source_chunks[0], &mut app.source_list_state);
        f.render_widget(source_content, source_chunks[1]);
        return;
    }

    // Graph mode rendering
    if app.graph_mode {
        // Calculate visible window
        // Use -3 (1 header + 2 borders) to maximize visible rows.
        // We subtract an extra 1 to be safe against Table widget scrolling behavior when full.
        let height = area.height.saturating_sub(4) as usize; 
        
        // Ensure selection is in view
        if app.tree_selected < app.tree_scroll {
            app.tree_scroll = app.tree_selected;
        } else if app.tree_selected >= app.tree_scroll + height {
            app.tree_scroll = app.tree_selected.saturating_sub(height).saturating_add(1);
        }

        // Get visible rows from virtualized walker
        let (total_rows, visible_rows) = app.compute_tree_view(Some(app.tree_scroll..app.tree_scroll + height + 5));

        // Header with cumulative size for root
        let header_cells = vec![
            Cell::from("Name").style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Self").style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Cumulative").style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Idx").style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        let header = Row::new(header_cells).style(Style::default()).height(1);

        let mut table_rows: Vec<Row> = Vec::new();

        if total_rows == 0 {
            let row = Row::new(vec![
                Cell::from("No root selected (press g on a function)"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray));
            table_rows.push(row);
        } else {
            for r in visible_rows {
                // render indented name with expand/collapse marker
                let name = if let Some(f) = app.module.functions.iter().find(|f| f.index == r.index)
                {
                    display_name(f, app.raw_names)
                } else {
                    format!("func[{}]", r.index)
                };
                let marker = if app
                    .call_graph
                    .edges
                    .get(&r.index)
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
                {
                    if app.expanded.contains(&r.path) {
                        "[-] "
                    } else {
                        "[+] "
                    }
                } else {
                    "    "
                };
                let indirect_tag = if r.has_indirect { " [indirect]" } else { "" };
                let cycle_tag = if r.is_cycle { " [cycle]" } else { "" };
                let indent = "  ".repeat(r.depth);
                let display = format!("{}{}{}{}{}", indent, marker, name, indirect_tag, cycle_tag);

                // self size
                let self_size = app
                    .module
                    .functions
                    .iter()
                    .find(|f| f.index == r.index)
                    .map(|f| f.code_size)
                    .unwrap_or(0);

                // cumulative unique size
                // This is the expensive call we only want to do for visible rows
                let (cum, _) = unique_cumulative_size(r.index, &app.module, &app.call_graph);

                let cells = vec![
                    Cell::from(display),
                    Cell::from(format!("{}", self_size)),
                    Cell::from(format!("{}", cum)),
                    Cell::from(format!("{}", r.index)),
                ];
                table_rows.push(Row::new(cells).height(1));
            }
        }

        let widths = [
            Constraint::Min(20),    // Name
            Constraint::Length(12), // Self
            Constraint::Length(14), // Cumulative
            Constraint::Length(8),  // Idx
        ];

        let table = Table::new(table_rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Call Graph (g to toggle) "),
            )
            .row_highlight_style(Style::default().fg(Color::Black).bg(Color::White))
            .column_spacing(1);

        let mut state = TableState::default();
        if total_rows > 0 {
            // Selection is relative to the visible window in the table widget 
            // if we feed it only the visible rows? 
            // Actually, Table widget expects selection index to match the row index in the provided vector.
            // Since we are providing a slice starting from `tree_scroll`, the relative index is:
            let rel_sel = app.tree_selected.saturating_sub(app.tree_scroll);
            state.select(Some(rel_sel));
        }
        f.render_stateful_widget(table, area, &mut state);
        return;
    }

    // Default list mode rendering
    let header_cells = ["Rank", "%", "Size", "Index", "Name"].into_iter().map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells).style(Style::default()).height(1);

    let mut rows: Vec<Row> = Vec::new();

    // Calculate visible window for main view
    // Use -3 (1 header + 2 borders) to maximize visible rows.
    // We subtract an extra 1 to be safe against Table widget scrolling behavior when full.
    let height = area.height.saturating_sub(4) as usize;

    // Ensure selection is in view
    if app.selected < app.main_scroll {
        app.main_scroll = app.selected;
    } else if app.selected >= app.main_scroll + height {
        app.main_scroll = app.selected.saturating_sub(height).saturating_add(1);
    }

    let visible_indices = app.indices.iter().skip(app.main_scroll).take(height);

    if app.indices.is_empty() {
        let row = Row::new(vec![
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from("No functions (or no matches)"),
        ])
        .style(Style::default().fg(Color::DarkGray));
        rows.push(row);
    } else {
        for (rank_offset, idx) in visible_indices.enumerate() {
            let rank = app.main_scroll + rank_offset;
            let f = &app.module.functions[*idx];
            let pct = app.module.percentage(f);
            let name = display_name(f, app.raw_names);
            let cells = vec![
                Cell::from(format!("{}", rank + 1)),
                Cell::from(format!("{:.2}", pct)),
                Cell::from(format!("{}", f.code_size)),
                Cell::from(format!("{}", f.index)),
                Cell::from(name),
            ];
            rows.push(Row::new(cells).height(1));
        }
    }

    let widths = [
        Constraint::Length(6),  // Rank
        Constraint::Length(7),  // %
        Constraint::Length(12), // Size
        Constraint::Length(8),  // Index
        Constraint::Min(10),    // Name
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Functions by size (desc) "),
        )
        .row_highlight_style(Style::default().fg(Color::Black).bg(Color::White))
        .column_spacing(1);

    let mut state = TableState::default();
    if !app.indices.is_empty() {
        let rel_sel = app.selected.saturating_sub(app.main_scroll);
        state.select(Some(rel_sel));
    }
    f.render_stateful_widget(table, area, &mut state);

    // Draw details for selected at the bottom-right corner of the table area (overlay)
    if let Some(func) = app.selected_function() {
        let details = vec![
            Line::from(Span::raw(format!("Index: {}", func.index))),
            Line::from(Span::raw(format!("Size: {}", func.code_size))),
            Line::from(Span::raw(format!(
                "Percent: {:.2}%",
                app.module.percentage(func)
            ))),
            Line::from(Span::raw(format!(
                "Name: {}",
                display_name(func, app.raw_names)
            ))),
            Line::from(Span::raw(format!(
                "Raw: {}",
                func.raw_name
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("<none>")
            ))),
            Line::from(Span::raw(format!(
                "Demangled: {}",
                func.demangled_name
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("<none>")
            ))),
            Line::from(Span::raw(format!(
                "Exports: {}",
                if func.export_names.is_empty() {
                    "<none>".to_string()
                } else {
                    func.export_names.join(", ")
                }
            ))),
        ];
        let detail_block = Block::default().borders(Borders::ALL).title(" Selected ");
        // Place it in a small overlay in bottom right of the functions area
        let overlay = Rect {
            x: area.x + area.width.saturating_sub(40),
            y: area.y + area.height.saturating_sub(9),
            width: 40.min(area.width),
            height: 9.min(area.height),
        };

        let para = Paragraph::new(details)
            .block(detail_block)
            .wrap(Wrap { trim: true });

        // Clear background under overlay for readability
        f.render_widget(ratatui::widgets::Clear, overlay);
        f.render_widget(para, overlay);
    }
}

fn draw_footer(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Help ");

    if app.in_search {
        // Inline input hint and live filter text while typing
        let help = Line::from(vec![
            Span::styled(
                "Filter: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.filter.clone()),
            Span::raw("  "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" apply  "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" cancel  "),
            Span::styled(
                "Ctrl+U",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" clear  "),
            Span::raw("(*) wildcard"),
        ]);
        let p = Paragraph::new(help).block(block);
        f.render_widget(p, area);
        return;
    }

    let help = Line::from(vec![
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" quit  "),
        Span::styled(
            "/",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" filter  "),
        Span::styled(
            "g",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.inspect_mode {
            " GOTO  "
        } else {
            " graph  "
        }),
        Span::styled(
            "i",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" inspect  "),
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" clear  "),
        Span::styled(
            "r",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.raw_names {
            " names: raw "
        } else {
            " names: best "
        }),
        Span::raw("  "),
        Span::styled(
            "j/k/↑/↓/PgUp/PgDn/Home/End",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.graph_mode {
            if app.inspect_mode {
                " navigate (h/l collapse/expand, Enter expand, j/k line, PgUp/PgDn page)"
            } else {
                " navigate (h/l collapse/expand, Enter expand)"
            }
        } else {
            if app.inspect_mode {
                " navigate (j/k line, PgUp/PgDn page)"
            } else {
                " navigate"
            }
        }),
    ]);
    let p = Paragraph::new(help).block(block);
    f.render_widget(p, area);
}

fn display_name(f: &FunctionInfo, raw_names: bool) -> String {
    if raw_names {
        if let Some(r) = &f.raw_name {
            return r.clone();
        }
    }
    f.best_name()
}

fn collect_sorted_indices(module: &WasmModuleInfo, pattern: Option<&str>) -> Vec<usize> {
    // default to substring contains when no '*' is present by wrapping as *pat*
    let normalized: Option<String> = pattern.map(|p| {
        if p.contains('*') {
            p.to_string()
        } else {
            format!("*{}*", p)
        }
    });
    let normalized_ref = normalized.as_deref();

    let mut idxs: Vec<usize> = Vec::with_capacity(module.functions.len());
    for (i, f) in module.functions.iter().enumerate() {
        if let Some(p) = normalized_ref {
            if !function_matches(f, p) {
                continue;
            }
        }
        idxs.push(i);
    }
    idxs.sort_by_key(|i| std::cmp::Reverse(module.functions[*i].code_size));
    idxs
}

#[derive(Debug, Serialize)]
struct JsonOutput {
    wasm_path: String,
    total_code_size: u64,
    imported_functions: u32,
    defined_functions: u32,
    functions: Vec<JsonFunction>,
}

#[derive(Debug, Serialize)]
struct JsonFunction {
    index: u32,
    size: u32,
    percent: f64,
    raw_name: Option<String>,
    demangled_name: Option<String>,
    export_names: Vec<String>,
    display_name: String,
}

fn collect_output_items(
    module: &WasmModuleInfo,
    pattern: Option<&str>,
    raw_names: bool,
) -> Vec<JsonFunction> {
    let mut out = Vec::new();
    for i in collect_sorted_indices(module, pattern) {
        let f = &module.functions[i];
        out.push(JsonFunction {
            index: f.index,
            size: f.code_size,
            percent: module.percentage(f),
            raw_name: f.raw_name.clone(),
            demangled_name: f.demangled_name.clone(),
            export_names: f.export_names.clone(),
            display_name: display_name(f, raw_names),
        });
    }
    out
}


