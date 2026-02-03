use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod app;
mod config;
mod db;
mod parser;
mod ui;

use app::{App, Focus};
use config::Config;
use db::SecondaryDb;

#[derive(Parser, Debug)]
#[command(name = "rocksdb-tui")]
#[command(about = "A TUI browser for RocksDB databases")]
struct Args {
    /// Path to RocksDB database directory
    #[arg(long, required = true)]
    db: PathBuf,

    /// Path to parser config file (TOML)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Path for secondary instance (default: /tmp/rocksdb-tui-<hash>)
    #[arg(long)]
    secondary: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = match &args.config {
        Some(path) => Config::load(path)?,
        None => Config::default(),
    };

    let db = SecondaryDb::open(&args.db, args.secondary.as_deref())?;
    let mut app = App::new(db, config)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    const DEBOUNCE_MS: u64 = 200;
    let mut pending_search: Option<Instant> = None;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Check if we have a pending search that's ready to execute
        if let Some(last_input) = pending_search {
            if last_input.elapsed() >= Duration::from_millis(DEBOUNCE_MS) {
                app.execute_search()?;
                pending_search = None;
            }
        }

        // Poll for events with timeout
        let timeout = if pending_search.is_some() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(100)
        };

        if !event::poll(timeout)? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if app.search_active {
                match key.code {
                    KeyCode::Esc => {
                        pending_search = None;
                        app.clear_search()?;
                    }
                    KeyCode::Enter | KeyCode::Tab => {
                        // Execute immediately and exit search mode
                        if pending_search.is_some() {
                            app.execute_search()?;
                            pending_search = None;
                        }
                        app.search_active = false;
                        if key.code == KeyCode::Tab {
                            app.next_focus();
                        }
                    }
                    KeyCode::BackTab => {
                        if pending_search.is_some() {
                            app.execute_search()?;
                            pending_search = None;
                        }
                        app.search_active = false;
                        app.prev_focus();
                    }
                    KeyCode::Backspace => {
                        app.search_input.pop();
                        pending_search = Some(Instant::now());
                    }
                    KeyCode::Char(c) => {
                        app.search_input.push(c);
                        pending_search = Some(Instant::now());
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Tab => {
                        app.next_focus();
                    }
                    KeyCode::BackTab => {
                        app.prev_focus();
                    }
                    KeyCode::Char('/') => {
                        app.search_active = true;
                        app.focus = Focus::Keys;
                    }
                    KeyCode::Esc => {
                        if app.search_prefix.is_some() {
                            app.clear_search()?;
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => match app.focus {
                        Focus::ColumnFamilies => {
                            app.next_cf()?;
                        }
                        Focus::Keys => {
                            app.next_key()?;
                        }
                        Focus::Value => {}
                    },
                    KeyCode::Char('k') | KeyCode::Up => match app.focus {
                        Focus::ColumnFamilies => {
                            app.prev_cf()?;
                        }
                        Focus::Keys => {
                            app.prev_key();
                        }
                        Focus::Value => {}
                    },
                    KeyCode::Char('g') => {
                        app.first_key();
                    }
                    KeyCode::Char('G') => {
                        app.last_key()?;
                    }
                    KeyCode::Char('?') => {
                        // TODO: Show help popup
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
