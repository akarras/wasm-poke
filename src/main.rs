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
use wasm_poke::{function_matches, parse_wasm, FunctionInfo, WasmModuleInfo};

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

    if cli.json || cli.no_ui {
        run_non_interactive(&cli, &module)?;
        return Ok(());
    }

    run_tui(&cli, &module)
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
}

impl App {
    fn new(path: String, module: WasmModuleInfo, filter: Option<String>, raw_names: bool) -> Self {
        let mut app = Self {
            wasm_path: path,
            indices: Vec::new(),
            selected: 0,
            filter: filter.unwrap_or_default(),
            in_search: false,
            module,
            raw_names,
            last_update: Instant::now(),
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

        match key.code {
            KeyCode::Char('q') => return true,
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
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < self.indices.len() {
                    self.selected += 1;
                }
            }
            KeyCode::PageUp => {
                let step = 10usize;
                if self.selected >= step {
                    self.selected -= step;
                } else {
                    self.selected = 0;
                }
            }
            KeyCode::PageDown => {
                let step = 10usize;
                self.selected = (self.selected + step).min(self.indices.len().saturating_sub(1));
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
}

fn run_tui(cli: &Cli, module: &WasmModuleInfo) -> Result<()> {
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

fn draw_table(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
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

    // Default help text (dynamic name mode indicator)
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
        Span::raw(" navigate"),
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
