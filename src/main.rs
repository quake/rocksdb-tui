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
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if app.search_active {
                match key.code {
                    KeyCode::Esc => {
                        app.clear_search()?;
                    }
                    KeyCode::Enter => {
                        app.execute_search()?;
                    }
                    KeyCode::Backspace => {
                        app.search_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.search_input.push(c);
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
