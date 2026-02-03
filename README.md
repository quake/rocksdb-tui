# rocksdb-tui

A TUI browser for RocksDB databases using Secondary mode (read-only, safe).

## Features

- Connect to any local RocksDB in read-only Secondary mode (no locks, safe for production)
- Three-column layout: Column Families / Keys / Values
- Live prefix search with debounce (200ms)
- Refresh data from primary database without restart
- Cursor-based pagination for large datasets (100 keys per page)
- Multiple value format parsers:
  - **String** - UTF-8 text
  - **Hex** - Hexadecimal dump
  - **JSON** - Pretty-printed JSON
  - **MessagePack** - Binary msgpack to JSON
  - **Protobuf** - Dynamic protobuf decoding (no protoc required)
  - **Molecule** - CKB Molecule format decoding

## Quick Start

```bash
# Create a test database with sample data
cargo run --example create_test_db

# Run the TUI with the test database
cargo run -- --db /tmp/rocksdb-tui-test-db --config examples/test-config.toml
```

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
# String/JSON values
[[column_families]]
name = "users"
key_format = "string"      # string, hex, u64_be, u64_le
value_format = "json"      # string, hex, json, msgpack, protobuf, molecule

# MessagePack values
[[column_families]]
name = "sessions"
value_format = "msgpack"

# Protobuf values (dynamic decoding, no protoc needed)
[[column_families]]
name = "orders"
value_format = "protobuf"
proto_file = "protos/order.proto"
proto_message = "Order"
proto_includes = ["protos/"]  # optional: additional import paths

# Molecule values (CKB format)
[[column_families]]
name = "accounts"
value_format = "molecule"
mol_file = "schemas/types.mol"
mol_type = "Account"
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch focus between panels |
| `j` / `k` or `↑` / `↓` | Navigate up/down |
| `g` / `G` | Jump to first/last key |
| `/` | Start prefix search |
| `Esc` | Clear search filter |
| `r` | Refresh data from primary |
| `q` | Quit |

## How Secondary Mode Works

RocksDB Secondary mode opens the database as a read-only replica:

- No write locks - safe to use while primary is running
- Call `try_catch_up_with_primary()` to sync latest data (press `r`)
- Uses a separate secondary path for SST file tracking

## License

MIT
