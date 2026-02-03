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
