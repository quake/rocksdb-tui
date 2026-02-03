# RocksDB TUI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a read-only RocksDB TUI browser using Secondary mode with configurable data parsers.

**Architecture:** Three-layer design (TUI -> Parser -> Database). Secondary mode for safe read-only access. TOML config for parser rules. Cursor-based pagination with RocksDB iterators.

**Tech Stack:** Rust, ratatui, rust-rocksdb, clap, serde, crossterm

---

## Task 1: Project Setup

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "rocksdb-tui"
version = "0.1.0"
edition = "2021"

[dependencies]
rocksdb = "0.22"
ratatui = "0.28"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
anyhow = "1"
```

**Step 2: Create minimal main.rs**

```rust
fn main() {
    println!("rocksdb-tui");
}
```

**Step 3: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add Cargo.toml src/main.rs
git commit -m "chore: initialize project with dependencies"
```

---

## Task 2: CLI Argument Parsing

**Files:**
- Modify: `src/main.rs`

**Step 1: Implement CLI with clap**

```rust
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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
    
    println!("Database: {:?}", args.db);
    println!("Config: {:?}", args.config);
    println!("Secondary: {:?}", args.secondary);
    
    Ok(())
}
```

**Step 2: Verify CLI works**

Run: `cargo run -- --db /tmp/test`
Expected: Prints "Database: /tmp/test", Config: None, Secondary: None

Run: `cargo run`
Expected: Error about missing --db argument

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add CLI argument parsing with clap"
```

---

## Task 3: Config File Parsing

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`

**Step 1: Create config.rs**

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub column_families: Vec<ColumnFamilyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnFamilyConfig {
    pub name: String,
    #[serde(default = "default_key_format")]
    pub key_format: KeyFormat,
    #[serde(default = "default_value_format")]
    pub value_format: ValueFormat,
    pub proto_file: Option<String>,
    pub proto_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeyFormat {
    String,
    #[default]
    Hex,
    U64Be,
    U64Le,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValueFormat {
    String,
    #[default]
    Hex,
    Json,
    Msgpack,
    Protobuf,
}

fn default_key_format() -> KeyFormat {
    KeyFormat::Hex
}

fn default_value_format() -> ValueFormat {
    ValueFormat::Hex
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;
        Ok(config)
    }

    pub fn get_cf_config(&self, name: &str) -> Option<&ColumnFamilyConfig> {
        self.column_families.iter().find(|cf| cf.name == name)
    }
}
```

**Step 2: Update main.rs to use config**

```rust
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod config;

use config::Config;

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
    
    println!("Database: {:?}", args.db);
    println!("Config: {:?}", config);
    
    Ok(())
}
```

**Step 3: Test with sample config**

Create test config `/tmp/test-config.toml`:
```toml
[[column_families]]
name = "users"
key_format = "string"
value_format = "json"
```

Run: `cargo run -- --db /tmp/test --config /tmp/test-config.toml`
Expected: Prints config with users CF

**Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add TOML config file parsing"
```

---

## Task 4: Database Layer - Secondary Mode Connection

**Files:**
- Create: `src/db/mod.rs`
- Create: `src/db/secondary.rs`
- Modify: `src/main.rs`

**Step 1: Create src/db/mod.rs**

```rust
mod secondary;

pub use secondary::SecondaryDb;
```

**Step 2: Create src/db/secondary.rs**

```rust
use anyhow::{Context, Result};
use rocksdb::{DBWithThreadMode, MultiThreaded, Options};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct SecondaryDb {
    db: DBWithThreadMode<MultiThreaded>,
    column_families: Vec<String>,
}

impl SecondaryDb {
    pub fn open(db_path: &Path, secondary_path: Option<&Path>) -> Result<Self> {
        let secondary_path = match secondary_path {
            Some(p) => p.to_path_buf(),
            None => Self::default_secondary_path(db_path),
        };

        // Create secondary directory if not exists
        std::fs::create_dir_all(&secondary_path)
            .with_context(|| format!("Failed to create secondary path: {:?}", secondary_path))?;

        // List existing column families
        let cf_names = DBWithThreadMode::<MultiThreaded>::list_cf(&Options::default(), db_path)
            .unwrap_or_else(|_| vec!["default".to_string()]);

        // Open as secondary
        let mut opts = Options::default();
        opts.create_if_missing(false);

        let cf_descriptors: Vec<_> = cf_names
            .iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(name, Options::default()))
            .collect();

        let db = DBWithThreadMode::<MultiThreaded>::open_cf_descriptors_as_secondary(
            &opts,
            db_path,
            &secondary_path,
            cf_descriptors,
        )
        .with_context(|| format!("Failed to open database as secondary: {:?}", db_path))?;

        Ok(Self {
            db,
            column_families: cf_names,
        })
    }

    fn default_secondary_path(db_path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        db_path.hash(&mut hasher);
        let hash = hasher.finish();
        PathBuf::from(format!("/tmp/rocksdb-tui-{:x}", hash))
    }

    pub fn column_families(&self) -> &[String] {
        &self.column_families
    }

    pub fn try_catch_up_with_primary(&self) -> Result<()> {
        self.db
            .try_catch_up_with_primary()
            .context("Failed to catch up with primary")?;
        Ok(())
    }

    pub fn get(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .with_context(|| format!("Column family not found: {}", cf_name))?;
        let value = self.db.get_cf(&cf, key)?;
        Ok(value)
    }

    pub fn estimate_num_keys(&self, cf_name: &str) -> Option<u64> {
        let cf = self.db.cf_handle(cf_name)?;
        self.db
            .property_int_value_cf(&cf, "rocksdb.estimate-num-keys")
            .ok()
            .flatten()
    }

    pub fn iter_keys(&self, cf_name: &str, start_key: Option<&[u8]>, limit: usize) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .with_context(|| format!("Column family not found: {}", cf_name))?;

        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        
        let mut results = Vec::with_capacity(limit);
        let mut iter = iter.into_iter();

        // Skip to start_key if provided
        if let Some(start) = start_key {
            let iter_from = self.db.iterator_cf(&cf, rocksdb::IteratorMode::From(start, rocksdb::Direction::Forward));
            let mut iter_from = iter_from.into_iter();
            // Skip the start_key itself
            iter_from.next();
            for item in iter_from.take(limit) {
                let (k, v) = item?;
                results.push((k.to_vec(), v.to_vec()));
            }
        } else {
            for item in iter.take(limit) {
                let (k, v) = item?;
                results.push((k.to_vec(), v.to_vec()));
            }
        }

        Ok(results)
    }
}
```

**Step 3: Update main.rs**

```rust
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod config;
mod db;

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
    
    println!("Connected to database: {:?}", args.db);
    println!("Column families: {:?}", db.column_families());
    
    Ok(())
}
```

**Step 4: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/db/mod.rs src/db/secondary.rs src/main.rs
git commit -m "feat: add database layer with secondary mode connection"
```

---

## Task 5: Parser Layer

**Files:**
- Create: `src/parser/mod.rs`
- Create: `src/parser/key.rs`
- Create: `src/parser/value.rs`
- Modify: `src/main.rs`

**Step 1: Create src/parser/mod.rs**

```rust
mod key;
mod value;

pub use key::parse_key;
pub use value::parse_value;
```

**Step 2: Create src/parser/key.rs**

```rust
use crate::config::KeyFormat;

pub fn parse_key(data: &[u8], format: &KeyFormat) -> String {
    match format {
        KeyFormat::String => String::from_utf8_lossy(data).to_string(),
        KeyFormat::Hex => hex_encode(data),
        KeyFormat::U64Be => {
            if data.len() == 8 {
                let arr: [u8; 8] = data.try_into().unwrap();
                u64::from_be_bytes(arr).to_string()
            } else {
                hex_encode(data)
            }
        }
        KeyFormat::U64Le => {
            if data.len() == 8 {
                let arr: [u8; 8] = data.try_into().unwrap();
                u64::from_le_bytes(arr).to_string()
            } else {
                hex_encode(data)
            }
        }
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
```

**Step 3: Create src/parser/value.rs**

```rust
use crate::config::ValueFormat;

pub struct ParseResult {
    pub content: String,
    pub success: bool,
}

pub fn parse_value(data: &[u8], format: &ValueFormat) -> ParseResult {
    match format {
        ValueFormat::String => parse_string(data),
        ValueFormat::Hex => ParseResult {
            content: hex_encode(data),
            success: true,
        },
        ValueFormat::Json => parse_json(data),
        ValueFormat::Msgpack => ParseResult {
            content: format!("msgpack: {} bytes (not implemented)", data.len()),
            success: false,
        },
        ValueFormat::Protobuf => ParseResult {
            content: format!("protobuf: {} bytes (not implemented)", data.len()),
            success: false,
        },
    }
}

fn parse_string(data: &[u8]) -> ParseResult {
    match String::from_utf8(data.to_vec()) {
        Ok(s) => ParseResult {
            content: s,
            success: true,
        },
        Err(_) => ParseResult {
            content: hex_encode(data),
            success: false,
        },
    }
}

fn parse_json(data: &[u8]) -> ParseResult {
    match serde_json::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => ParseResult {
                content: s,
                success: true,
            },
            Err(_) => ParseResult {
                content: hex_encode(data),
                success: false,
            },
        },
        Err(_) => ParseResult {
            content: hex_encode(data),
            success: false,
        },
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
```

**Step 4: Update main.rs to include parser module**

Add after `mod db;`:

```rust
mod parser;
```

**Step 5: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add src/parser/mod.rs src/parser/key.rs src/parser/value.rs src/main.rs
git commit -m "feat: add parser layer for key and value formatting"
```

---

## Task 6: App State Management

**Files:**
- Create: `src/app.rs`
- Modify: `src/main.rs`

**Step 1: Create src/app.rs**

```rust
use crate::config::{Config, KeyFormat, ValueFormat};
use crate::db::SecondaryDb;
use crate::parser::{parse_key, parse_value};
use anyhow::Result;

const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    ColumnFamilies,
    Keys,
    Value,
}

pub struct App {
    pub db: SecondaryDb,
    pub config: Config,
    pub focus: Focus,
    pub cf_index: usize,
    pub key_index: usize,
    pub keys: Vec<(Vec<u8>, Vec<u8>)>,
    pub has_more_keys: bool,
    pub search_input: String,
    pub search_active: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(db: SecondaryDb, config: Config) -> Result<Self> {
        let mut app = Self {
            db,
            config,
            focus: Focus::ColumnFamilies,
            cf_index: 0,
            key_index: 0,
            keys: Vec::new(),
            has_more_keys: false,
            search_input: String::new(),
            search_active: false,
            should_quit: false,
        };
        app.load_keys()?;
        Ok(app)
    }

    pub fn current_cf(&self) -> Option<&str> {
        self.db.column_families().get(self.cf_index).map(|s| s.as_str())
    }

    pub fn key_format(&self) -> KeyFormat {
        self.current_cf()
            .and_then(|cf| self.config.get_cf_config(cf))
            .map(|c| c.key_format.clone())
            .unwrap_or_default()
    }

    pub fn value_format(&self) -> ValueFormat {
        self.current_cf()
            .and_then(|cf| self.config.get_cf_config(cf))
            .map(|c| c.value_format.clone())
            .unwrap_or_default()
    }

    pub fn load_keys(&mut self) -> Result<()> {
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            self.keys = self.db.iter_keys(&cf, None, PAGE_SIZE)?;
            self.has_more_keys = self.keys.len() == PAGE_SIZE;
            self.key_index = 0;
        }
        Ok(())
    }

    pub fn load_more_keys(&mut self) -> Result<()> {
        if !self.has_more_keys {
            return Ok(());
        }
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            if let Some((last_key, _)) = self.keys.last() {
                let more = self.db.iter_keys(&cf, Some(last_key), PAGE_SIZE)?;
                self.has_more_keys = more.len() == PAGE_SIZE;
                self.keys.extend(more);
            }
        }
        Ok(())
    }

    pub fn estimate_keys(&self) -> Option<u64> {
        self.current_cf()
            .and_then(|cf| self.db.estimate_num_keys(cf))
    }

    pub fn formatted_keys(&self) -> Vec<String> {
        let format = self.key_format();
        self.keys
            .iter()
            .map(|(k, _)| parse_key(k, &format))
            .collect()
    }

    pub fn current_value(&self) -> Option<(String, bool)> {
        self.keys.get(self.key_index).map(|(_, v)| {
            let result = parse_value(v, &self.value_format());
            (result.content, result.success)
        })
    }

    pub fn next_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = (self.cf_index + 1) % len;
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn prev_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = if self.cf_index == 0 {
                len - 1
            } else {
                self.cf_index - 1
            };
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn next_key(&mut self) -> Result<()> {
        if self.key_index + 1 < self.keys.len() {
            self.key_index += 1;
        } else if self.has_more_keys {
            self.load_more_keys()?;
            if self.key_index + 1 < self.keys.len() {
                self.key_index += 1;
            }
        }
        Ok(())
    }

    pub fn prev_key(&mut self) {
        if self.key_index > 0 {
            self.key_index -= 1;
        }
    }

    pub fn first_key(&mut self) {
        self.key_index = 0;
    }

    pub fn last_key(&mut self) -> Result<()> {
        // Load all remaining keys
        while self.has_more_keys {
            self.load_more_keys()?;
        }
        if !self.keys.is_empty() {
            self.key_index = self.keys.len() - 1;
        }
        Ok(())
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Keys,
            Focus::Keys => Focus::Value,
            Focus::Value => Focus::ColumnFamilies,
        };
    }

    pub fn prev_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Value,
            Focus::Keys => Focus::ColumnFamilies,
            Focus::Value => Focus::Keys,
        };
    }
}
```

**Step 2: Update main.rs**

Add `mod app;` after other mod declarations.

**Step 3: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat: add app state management"
```

---

## Task 7: TUI Layout and Rendering

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/layout.rs`
- Modify: `src/main.rs`

**Step 1: Create src/ui/mod.rs**

```rust
mod layout;

pub use layout::draw;
```

**Step 2: Create src/ui/layout.rs**

```rust
use crate::app::{App, Focus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    // Three column layout
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(main_area);

    draw_cf_list(frame, app, columns[0]);
    draw_key_list(frame, app, columns[1]);
    draw_value_view(frame, app, columns[2]);
    draw_status_bar(frame, app, status_area);
}

fn draw_cf_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::ColumnFamilies;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = app
        .db
        .column_families()
        .iter()
        .map(|cf| ListItem::new(cf.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Column Families")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.cf_index));

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_key_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Keys;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    // Split area for search box and key list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search box
    let search_style = if app.search_active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let search_text = if app.search_input.is_empty() && !app.search_active {
        "Press / to search..."
    } else {
        &app.search_input
    };
    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(Block::default().title("Search").borders(Borders::ALL));
    frame.render_widget(search, chunks[0]);

    // Key list
    let keys = app.formatted_keys();
    let mut items: Vec<ListItem> = keys.iter().map(|k| ListItem::new(k.as_str())).collect();

    if app.has_more_keys {
        items.push(ListItem::new("▼ more...").style(Style::default().fg(Color::DarkGray)));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title("Keys")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.key_index));

    frame.render_stateful_widget(list, chunks[1], &mut state);
}

fn draw_value_view(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Value;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let (content, success) = app.current_value().unwrap_or_default();

    let mut lines = Vec::new();
    if !success {
        lines.push(Line::from(Span::styled(
            "⚠ Parse failed, showing hex",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
    }
    for line in content.lines() {
        lines.push(Line::from(line));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Value")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let cf_name = app.current_cf().unwrap_or("N/A");
    let key_count = app
        .estimate_keys()
        .map(|n| format!("~{}", format_number(n)))
        .unwrap_or_else(|| "?".to_string());
    let format = format!("{:?}", app.value_format());

    let status = format!(
        " CF: {} | Keys: {} (estimate) | Format: {} | [?] Help",
        cf_name, key_count, format
    );

    let paragraph = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
```

**Step 3: Update main.rs to include ui module**

Add `mod ui;` after other mod declarations.

**Step 4: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/ui/mod.rs src/ui/layout.rs src/main.rs
git commit -m "feat: add TUI layout with three-column view"
```

---

## Task 8: Event Handling and Main Loop

**Files:**
- Modify: `src/main.rs`

**Step 1: Implement main event loop**

```rust
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
                        app.search_active = false;
                        app.search_input.clear();
                    }
                    KeyCode::Enter => {
                        // TODO: Implement search
                        app.search_active = false;
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
```

**Step 2: Verify build and basic run**

Run: `cargo build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add event handling and main TUI loop"
```

---

## Task 9: Create Test Database and End-to-End Test

**Files:**
- Create: `tests/integration_test.rs`
- Create: `examples/create_test_db.rs`

**Step 1: Create examples/create_test_db.rs**

```rust
use rocksdb::{Options, DB};
use std::path::Path;

fn main() {
    let path = Path::new("/tmp/rocksdb-tui-test-db");
    
    // Remove old database if exists
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let cfs = vec!["default", "users", "logs"];
    let db = DB::open_cf(&opts, path, &cfs).unwrap();

    // Add some test data to default CF
    db.put(b"key1", b"value1").unwrap();
    db.put(b"key2", b"value2").unwrap();

    // Add JSON data to users CF
    let users_cf = db.cf_handle("users").unwrap();
    db.put_cf(&users_cf, b"user:1001", br#"{"name":"Alice","email":"alice@test.com"}"#).unwrap();
    db.put_cf(&users_cf, b"user:1002", br#"{"name":"Bob","email":"bob@test.com"}"#).unwrap();
    db.put_cf(&users_cf, b"user:1003", br#"{"name":"Charlie","email":"charlie@test.com"}"#).unwrap();

    // Add some logs
    let logs_cf = db.cf_handle("logs").unwrap();
    for i in 0..200 {
        let key = format!("log:{:05}", i);
        let value = format!("Log entry {}", i);
        db.put_cf(&logs_cf, key.as_bytes(), value.as_bytes()).unwrap();
    }

    println!("Test database created at {:?}", path);
    println!("Column families: {:?}", cfs);
}
```

**Step 2: Add example to Cargo.toml**

Add to Cargo.toml:
```toml
[[example]]
name = "create_test_db"
```

**Step 3: Run example to create test database**

Run: `cargo run --example create_test_db`
Expected: "Test database created at /tmp/rocksdb-tui-test-db"

**Step 4: Create test config**

Create `examples/test-config.toml`:
```toml
[[column_families]]
name = "users"
key_format = "string"
value_format = "json"

[[column_families]]
name = "logs"
key_format = "string"
value_format = "string"
```

**Step 5: Manual test**

Run: `cargo run -- --db /tmp/rocksdb-tui-test-db --config examples/test-config.toml`
Expected: TUI opens, shows 3 column families, can navigate with j/k, Tab, q to quit

**Step 6: Commit**

```bash
git add examples/create_test_db.rs examples/test-config.toml Cargo.toml
git commit -m "feat: add test database generator and sample config"
```

---

## Task 10: Polish and Documentation

**Files:**
- Update: `README.md` (create if not exists)
- Update: `.gitignore`

**Step 1: Create .gitignore**

```
/target
Cargo.lock
```

**Step 2: Create README.md**

```markdown
# rocksdb-tui

A TUI browser for RocksDB databases using Secondary mode (read-only).

## Features

- Connect to any local RocksDB in read-only Secondary mode
- Three-column layout: Column Families / Keys / Values
- Configurable parsers for different data formats
- Cursor-based pagination for large datasets

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Basic usage
rocksdb-tui --db /path/to/rocksdb

# With parser config
rocksdb-tui --db /path/to/rocksdb --config ./parsers.toml

# Custom secondary path
rocksdb-tui --db /path/to/rocksdb --secondary /tmp/my-secondary
```

## Configuration

Create a TOML file to configure parsers for each column family:

```toml
[[column_families]]
name = "users"
key_format = "string"      # string, hex, u64_be, u64_le
value_format = "json"      # string, hex, json, msgpack, protobuf
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch focus between panels |
| `j` / `k` or `↑` / `↓` | Navigate up/down |
| `gg` / `G` | Jump to top/bottom |
| `/` | Search (placeholder) |
| `q` | Quit |
| `?` | Help (placeholder) |

## License

MIT
```

**Step 3: Commit**

```bash
git add .gitignore README.md
git commit -m "docs: add README and gitignore"
```

---

## Summary

After completing all tasks, you will have:

1. A working TUI application that connects to RocksDB in Secondary mode
2. Three-column layout with CF list, Key list, and Value view
3. Cursor-based pagination for handling large datasets
4. Configurable parsers (string, hex, json) via TOML config
5. Keyboard navigation (j/k, Tab, gg/G, q)
6. Search UI placeholder
7. Status bar with CF name, estimated keys, and format

MVP is complete and ready for iteration on P1 features (search, msgpack, protobuf).
