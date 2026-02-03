# Search Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add prefix-based key search using RocksDB's native seek operation.

**Architecture:** User types prefix in search box, presses Enter, app seeks to prefix and shows matching keys. Esc clears search and restores full list. Uses RocksDB iterator with prefix seek, stopping when keys no longer match prefix.

**Tech Stack:** Rust, RocksDB iterator with `IteratorMode::From`, ratatui for UI updates.

---

### Task 1: Add Prefix Iterator to Database Layer

**Files:**
- Modify: `src/db/secondary.rs:85-122`

**Step 1: Add `iter_keys_with_prefix` method**

Add this method after `iter_keys` in `SecondaryDb`:

```rust
pub fn iter_keys_with_prefix(
    &self,
    cf_name: &str,
    prefix: &[u8],
    limit: usize,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let cf = self
        .db
        .cf_handle(cf_name)
        .with_context(|| format!("Column family not found: {}", cf_name))?;

    let iter = self.db.iterator_cf(
        &cf,
        rocksdb::IteratorMode::From(prefix, rocksdb::Direction::Forward),
    );

    let mut results = Vec::with_capacity(limit);
    for item in iter.take(limit) {
        let (k, v) = item?;
        if !k.starts_with(prefix) {
            break;
        }
        results.push((k.to_vec(), v.to_vec()));
    }

    Ok(results)
}
```

**Step 2: Verify build passes**

Run: `cargo build`
Expected: Compiles with existing warnings only

**Step 3: Commit**

```bash
git add src/db/secondary.rs
git commit -m "feat: add prefix iterator to database layer"
```

---

### Task 2: Add Search State to App

**Files:**
- Modify: `src/app.rs:15-26` (struct fields)
- Modify: `src/app.rs:28-44` (new method)

**Step 1: Add `search_prefix` field to App struct**

Update the `App` struct to add the search_prefix field after `search_active`:

```rust
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
    pub search_prefix: Option<Vec<u8>>,
    pub should_quit: bool,
}
```

**Step 2: Initialize `search_prefix` in `App::new`**

Update the `App::new` constructor to initialize `search_prefix`:

```rust
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
    search_prefix: None,
    should_quit: false,
};
```

**Step 3: Verify build passes**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add search_prefix state to App"
```

---

### Task 3: Implement Search Methods in App

**Files:**
- Modify: `src/app.rs:67-75` (load_keys)
- Modify: `src/app.rs` (add new methods after load_more_keys)

**Step 1: Update `load_keys` to use prefix when set**

Replace the `load_keys` method:

```rust
pub fn load_keys(&mut self) -> Result<()> {
    if let Some(cf) = self.current_cf() {
        let cf = cf.to_string();
        self.keys = if let Some(ref prefix) = self.search_prefix {
            self.db.iter_keys_with_prefix(&cf, prefix, PAGE_SIZE)?
        } else {
            self.db.iter_keys(&cf, None, PAGE_SIZE)?
        };
        self.has_more_keys = self.keys.len() == PAGE_SIZE && self.search_prefix.is_none();
        self.key_index = 0;
    }
    Ok(())
}
```

**Step 2: Add `execute_search` method**

Add after `load_more_keys` method:

```rust
pub fn execute_search(&mut self) -> Result<()> {
    if self.search_input.is_empty() {
        self.clear_search()?;
    } else {
        self.search_prefix = Some(self.search_input.as_bytes().to_vec());
        self.search_active = false;
        self.load_keys()?;
    }
    Ok(())
}

pub fn clear_search(&mut self) -> Result<()> {
    self.search_prefix = None;
    self.search_input.clear();
    self.search_active = false;
    self.load_keys()?;
    Ok(())
}
```

**Step 3: Verify build passes**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: implement execute_search and clear_search methods"
```

---

### Task 4: Wire Up Event Handling

**Files:**
- Modify: `src/main.rs:77-95` (search event handling)

**Step 1: Update search key handling in main loop**

Replace the search handling block (inside `if app.search_active`):

```rust
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
}
```

**Step 2: Verify build passes**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up search event handling"
```

---

### Task 5: Update UI to Show Search State

**Files:**
- Modify: `src/ui/layout.rs:80-94` (search box styling)
- Modify: `src/ui/layout.rs:120-130` (status bar)

**Step 1: Update search box to show active prefix**

Replace the search box rendering section:

```rust
// Search box
let search_style = if app.search_active {
    Style::default().fg(Color::Yellow)
} else if app.search_prefix.is_some() {
    Style::default().fg(Color::Green)
} else {
    Style::default().fg(Color::DarkGray)
};
let search_text = if app.search_active {
    app.search_input.clone()
} else if let Some(ref prefix) = app.search_prefix {
    format!("Filter: {}", String::from_utf8_lossy(prefix))
} else if app.search_input.is_empty() {
    "Press / to search...".to_string()
} else {
    app.search_input.clone()
};
let search = Paragraph::new(search_text)
    .style(search_style)
    .block(Block::default().title("Search").borders(Borders::ALL));
frame.render_widget(search, chunks[0]);
```

**Step 2: Update status bar to show filtered count**

Find the status bar section and update to show filter status. Replace the status bar line construction:

```rust
let key_count = if app.search_prefix.is_some() {
    format!("Filtered: {}", app.keys.len())
} else {
    app.estimate_keys()
        .map(|n| format!("~{}", n))
        .unwrap_or_else(|| "?".to_string())
};

let status = format!(
    " CF: {} | Keys: {} | Format: {:?} | [?] Help [/] Search [q] Quit ",
    app.current_cf().unwrap_or("none"),
    key_count,
    app.value_format()
);
```

**Step 3: Verify build passes**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: update UI to show search state and filtered count"
```

---

### Task 6: Manual End-to-End Test

**Step 1: Create test database (if not exists)**

Run: `cargo run --example create_test_db`
Expected: Creates database at `/tmp/rocksdb-tui-test-db`

**Step 2: Run the TUI**

Run: `cargo run -- --db /tmp/rocksdb-tui-test-db --config examples/test-config.toml`

**Step 3: Test search functionality**

1. Press `Tab` to focus on Keys panel
2. Press `/` to activate search
3. Type `user:100` and press Enter
4. Verify: Only keys starting with `user:100` are shown
5. Verify: Status bar shows "Filtered: X"
6. Verify: Search box shows "Filter: user:100" in green
7. Press `Esc`
8. Verify: Full key list is restored
9. Press `q` to quit

**Step 4: Commit final state**

```bash
git add -A
git commit -m "feat: complete search feature implementation"
```

---

### Task 7: Push to Remote

**Step 1: Push changes**

Run: `git push origin master`
Expected: All commits pushed to GitHub

---

## Summary

| Task | Description |
|------|-------------|
| 1 | Add prefix iterator to database layer |
| 2 | Add search_prefix state to App |
| 3 | Implement execute_search and clear_search methods |
| 4 | Wire up event handling |
| 5 | Update UI to show search state |
| 6 | Manual end-to-end test |
| 7 | Push to remote |
