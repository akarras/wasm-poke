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
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Terminal;
use serde::Serialize;
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

    // Summary mode
    let mut matches = collect_sorted_indices(module, cli.filter.as_deref());
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

#[derive(Debug)]
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
    expanded: std::collections::HashSet<u32>,
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
            module,
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
        };
        app.refresh_indices();
        app
    }

    fn refresh_indices(&mut self) {
        self.indices = collect_sorted_indices(
            &self.module,
            if self.filter.is_empty() {
                None
            } else {
                Some(&self.filter)
            },
        );
        if self.selected >= self.indices.len() {
            self.selected = self.indices.len().saturating_sub(1);
        }
    }

    fn selected_function(&self) -> Option<&FunctionInfo> {
        self.indices
            .get(self.selected)
            .and_then(|i| self.module.functions.get(*i))
    }

    fn on_key(&mut self, key: KeyEvent) -> bool {
        // Returns true to request exit
        if key.kind != KeyEventKind::Press {
            return false;
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

        // Graph mode handling
        if self.graph_mode {
            match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('g') => {
                    // toggle off graph mode
                    self.graph_mode = false;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_sub(1);
                    } else {
                        if self.tree_selected > 0 {
                            self.tree_selected -= 1;
                        }
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.inspect_mode {
                        self.wat_cursor = self.wat_cursor.saturating_add(1);
                    } else {
                        // clamp to current visible rows length (computed in draw)
                        // safe fallback: increment, will be clamped in draw selection
                        self.tree_selected = self.tree_selected.saturating_add(1);
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    // collapse current node if expanded
                    if let Some(rows) = self.visible_tree_rows() {
                        if let Some(row) = rows.get(self.tree_selected) {
                            if self.expanded.remove(&row.index) {
                                // collapsed current node
                            } else {
                                // if not expanded, try to move selection to parent (upwards)
                                if self.tree_selected > 0 {
                                    self.tree_selected -= 1;
                                }
                            }
                        }
                    }
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => {
                    if let Some(rows) = self.visible_tree_rows() {
                        if let Some(row) = rows.get(self.tree_selected) {
                            // expand if it has children
                            if self
                                .call_graph
                                .edges
                                .get(&row.index)
                                .map(|v| !v.is_empty())
                                .unwrap_or(false)
                            {
                                self.expanded.insert(row.index);
                            }
                        }
                    }
                }
                KeyCode::Home => {
                    self.tree_selected = 0;
                }
                KeyCode::End => {
                    if let Some(rows) = self.visible_tree_rows() {
                        if !rows.is_empty() {
                            self.tree_selected = rows.len() - 1;
                        }
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
                        let step = 10usize;
                        if let Some(rows) = self.visible_tree_rows() {
                            self.tree_selected =
                                (self.tree_selected + step).min(rows.len().saturating_sub(1));
                        }
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
                        if let Some(rows) = self.visible_tree_rows() {
                            if let Some(row) = rows.get(self.tree_selected) {
                                self.wat_lines =
                                    disassemble_function_wat_lines(&self.wasm_bytes, row.index)
                                        .unwrap_or_default();
                            } else {
                                self.wat_lines.clear();
                            }
                        } else {
                            self.wat_lines.clear();
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
                // enter graph mode with current selection as root
                if let Some(f) = self.selected_function() {
                    self.graph_root = Some(f.index);
                    self.expanded.clear();
                    self.tree_selected = 0;
                    self.graph_mode = true;
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
                    self.wat_lines = disassemble_function_wat_lines(&self.wasm_bytes, func_index)
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
                } else {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.inspect_mode {
                    self.wat_cursor = self.wat_cursor.saturating_add(1);
                } else {
                    if self.selected + 1 < self.indices.len() {
                        self.selected += 1;
                    }
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
            }
            KeyCode::End => {
                if !self.indices.is_empty() {
                    self.selected = self.indices.len() - 1;
                }
            }
            _ => {}
        }
        false
    }

    // Compute visible tree rows based on root and expanded nodes.
    // Returns None if no root is set.
    fn visible_tree_rows(&self) -> Option<Vec<TreeRow>> {
        let root = self.graph_root?;
        let mut rows: Vec<TreeRow> = Vec::new();
        use std::collections::HashSet;
        let mut path: Vec<u32> = Vec::new();
        let mut stack: Vec<(u32, usize, usize)> = Vec::new(); // (node, depth, child_index)
        stack.push((root, 0, 0));
        path.push(root);

        // helper to push a row
        let mut push_row = |idx: u32, depth: usize, is_cycle: bool| {
            let has_indirect = *self.call_graph.has_indirect.get(&idx).unwrap_or(&false);
            rows.push(TreeRow {
                depth,
                index: idx,
                is_cycle,
                has_indirect,
            });
        };

        while let Some((node, depth, mut child_i)) = stack.pop() {
            // push this node (when first visiting)
            if child_i == 0 {
                push_row(node, depth, false);
            }

            // expand children only if expanded contains node
            if self.expanded.contains(&node) {
                if let Some(children) = self.call_graph.edges.get(&node) {
                    while child_i < children.len() {
                        let child = children[child_i];
                        child_i += 1;
                        // put current back with next child index
                        stack.push((node, depth, child_i));
                        if path.contains(&child) {
                            // cycle marker
                            push_row(child, depth + 1, true);
                            break;
                        } else {
                            // descend
                            path.push(child);
                            stack.push((child, depth + 1, 0));
                            break;
                        }
                    }
                    // when exhausted, pop from path
                    if child_i >= children.len() {
                        let _ = path.pop();
                    }
                } else {
                    // no children
                    let _ = path.pop();
                }
            } else {
                // node collapsed; pop from path
                let _ = path.pop();
            }
        }

        Some(rows)
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

fn draw_ui(f: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(f.size());

    draw_header(f, chunks[0], app);
    draw_table(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
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

#[derive(Debug, Clone)]
struct TreeRow {
    depth: usize,
    index: u32,
    is_cycle: bool,
    has_indirect: bool,
}

fn draw_table(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    // Inspect mode rendering (side-by-side hex and WAT)
    if app.inspect_mode {
        // Determine current function index based on current mode/selection
        let current_index = if app.graph_mode {
            if let Some(rows) = app.visible_tree_rows() {
                rows.get(app.tree_selected).map(|r| r.index)
            } else {
                None
            }
        } else {
            app.selected_function().map(|f| f.index)
        };

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

        // Prepare hex
        let hex_text = wasm_poke::hexdump(body_bytes, 16);
        // Structured WAT lines (indent + offsets + optional source)
        let wat_lines: Vec<WatLine> = if app.wat_lines.is_empty() {
            disassemble_function_wat_lines(&app.wasm_bytes, current_index).unwrap_or_default()
        } else {
            app.wat_lines.clone()
        };
        let total_wat_lines = wat_lines.len();
        let cursor = app.wat_cursor.min(total_wat_lines.saturating_sub(1));
        let mut wat_buf = String::new();
        for (i, wl) in wat_lines.iter().enumerate() {
            let marker = if i == cursor { ">" } else { " " };
            wat_buf.push_str(&format!("{marker} {}\n", wl.text));
        }
        let wat_text = wat_buf;

        // Split horizontally into three panes: Hex | WAT | Source
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);

        let hex_widget =
            Paragraph::new(hex_text).block(Block::default().borders(Borders::ALL).title(" Hex "));
        // Attempt to resolve a source location for this function (first instruction as heuristic)
        let loc_opt = wasm_poke::map_instr_to_source(&app.wasm_bytes, current_index, 0);
        // Prepend a source header to the WAT view if available
        let wat_text = if let Some(ref loc) = loc_opt {
            format!(
                ";; source: {}:{}:{}\n{}",
                loc.file, loc.line, loc.column, wat_text
            )
        } else {
            wat_text
        };
        // Center WAT cursor in pane (derive scroll purely from cursor)
        let visible_wat_lines: u16 = cols[1].height.saturating_sub(2);
        let vis = visible_wat_lines as usize;
        let desired_top = cursor.saturating_sub(vis / 2);
        let max_top = total_wat_lines.saturating_sub(vis);
        let wat_scroll_local = desired_top.min(max_top);
        let wat_scroll_u16 = wat_scroll_local as u16;

        let wat_widget = Paragraph::new(wat_text)
            .block(Block::default().borders(Borders::ALL).title(" WAT "))
            .scroll((wat_scroll_u16, 0));
        let span_opt = wasm_poke::function_source_span(&app.wasm_bytes, current_index);
        // Determine target source line from WAT cursor (fallback to function start)
        let target_line_opt: Option<u32> = wat_lines
            .get(cursor)
            .and_then(|w| w.src.as_ref().map(|s| s.line))
            .or_else(|| span_opt.as_ref().map(|sp| sp.start_line));

        let (source_text, source_scroll_u16) = if let Some(span) = span_opt {
            match std::fs::read_to_string(&span.file) {
                Ok(src) => {
                    let lines: Vec<&str> = src.lines().collect();
                    let start = (span.start_line as usize).saturating_sub(1);
                    let end = (span.end_line as usize).min(lines.len());
                    let mut buf = String::new();

                    // Build buffer and compute desired scroll to keep target line visible
                    let target_line = target_line_opt.unwrap_or(span.start_line);
                    let target_idx_abs = (target_line as usize).saturating_sub(1);
                    let rel_idx = target_idx_abs.saturating_sub(start);

                    for i in start..end {
                        let ln = i + 1;
                        let marker = if (ln as u32) == target_line { ">" } else { " " };
                        let content = lines[i];
                        buf.push_str(&format!("{marker} {:5} | {content}\n", ln));
                    }

                    let total_source_lines: u16 = buf.lines().count() as u16;
                    let visible_source_lines: u16 = cols[2].height.saturating_sub(2); // minus borders
                    let max_source_scroll: u16 = if total_source_lines > visible_source_lines {
                        total_source_lines - visible_source_lines
                    } else {
                        0
                    };
                    // Center target line when possible
                    let half = (visible_source_lines / 2) as usize;
                    let mut desired = if rel_idx > half {
                        (rel_idx - half) as u16
                    } else {
                        0
                    };
                    if desired > max_source_scroll {
                        desired = max_source_scroll;
                    }

                    (buf, desired)
                }
                Err(_) => {
                    let fallback = format!(
                        "{}:{}:{}-{}:{}",
                        span.file,
                        span.start_line,
                        span.start_column,
                        span.end_line,
                        span.end_column
                    );
                    (fallback, 0)
                }
            }
        } else if let Some(src_loc) = wat_lines.get(cursor).and_then(|w| w.src.as_ref()) {
            match std::fs::read_to_string(&src_loc.file) {
                Ok(src) => {
                    let lines: Vec<&str> = src.lines().collect();
                    let target_line = src_loc.line;
                    let center = (target_line as usize).max(1);
                    let start = center.saturating_sub(3);
                    let end = (center + 3).min(lines.len());
                    let mut buf = String::new();
                    for i in start..end {
                        let ln = i + 1;
                        let marker = if ln as u32 == target_line { ">" } else { " " };
                        let content = lines[i];
                        buf.push_str(&format!("{marker} {:5} | {content}\n", ln));
                    }

                    // Center highlighted line within the source pane when possible
                    let total_source_lines: u16 = buf.lines().count() as u16;
                    let visible_source_lines: u16 = cols[2].height.saturating_sub(2);
                    let max_source_scroll: u16 = if total_source_lines > visible_source_lines {
                        total_source_lines - visible_source_lines
                    } else {
                        0
                    };
                    let half = (visible_source_lines / 2) as usize;
                    let rel_idx = (center.saturating_sub(start + 1))
                        .min((total_source_lines as usize).saturating_sub(1));
                    let mut desired = if rel_idx > half {
                        (rel_idx - half) as u16
                    } else {
                        0
                    };
                    if desired > max_source_scroll {
                        desired = max_source_scroll;
                    }

                    (buf, desired)
                }
                Err(_) => (format!("{}:{}", src_loc.file, src_loc.line), 0),
            }
        } else {
            (
                "No source mapping available.\nBuild with debug information (DWARF) to enable source pane.".to_string(),
                0,
            )
        };

        let source_widget = Paragraph::new(source_text)
            .block(Block::default().borders(Borders::ALL).title(" Source "))
            .scroll((source_scroll_u16, 0));

        f.render_widget(hex_widget, cols[0]);
        f.render_widget(wat_widget, cols[1]);
        f.render_widget(source_widget, cols[2]);
        return;
    }

    // Graph mode rendering
    if app.graph_mode {
        // Build visible rows
        let mut rows = app.visible_tree_rows().unwrap_or_default();
        // Apply filter in call graph: keep rows that match or are ancestors of a match
        if !app.filter.is_empty() {
            let pat = if app.filter.contains('*') {
                app.filter.clone()
            } else {
                format!("*{}*", app.filter)
            };
            // Compute match flags per row
            let mut match_flags: Vec<bool> = Vec::with_capacity(rows.len());
            for r in &rows {
                let is_match =
                    if let Some(f) = app.module.functions.iter().find(|f| f.index == r.index) {
                        wasm_poke::function_matches(f, &pat)
                    } else {
                        false
                    };
                match_flags.push(is_match);
            }
            // Mark ancestors of matches
            let mut keep: Vec<bool> = vec![false; rows.len()];
            let mut stack: Vec<usize> = Vec::new(); // indices of ancestors by depth
            for (i, r) in rows.iter().enumerate() {
                while stack.len() > r.depth {
                    stack.pop();
                }
                if match_flags[i] {
                    keep[i] = true;
                    for &anc in &stack {
                        keep[anc] = true;
                    }
                }
                stack.push(i);
            }
            // Filter rows
            rows = rows
                .into_iter()
                .zip(keep.into_iter())
                .filter_map(|(r, k)| if k { Some(r) } else { None })
                .collect();
        }

        // Header with cumulative size for root
        let mut header_cells = vec![
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

        if rows.is_empty() {
            let row = Row::new(vec![
                Cell::from("No root selected (press g on a function)"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray));
            table_rows.push(row);
        } else {
            for r in &rows {
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
                    if app.expanded.contains(&r.index) {
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
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
            .column_spacing(1);

        let mut state = TableState::default();
        if !rows.is_empty() {
            let clamp = rows.len().saturating_sub(1);
            let sel = app.tree_selected.min(clamp);
            state.select(Some(sel));
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
        for (rank, idx) in app.indices.iter().enumerate() {
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
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
        .column_spacing(1);

    let mut state = TableState::default();
    if !app.indices.is_empty() {
        state.select(Some(app.selected));
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
        Span::raw(" graph  "),
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
