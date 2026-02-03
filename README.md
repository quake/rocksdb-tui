# rocksdb-tui

A TUI browser for RocksDB databases using Secondary mode (read-only, safe).

> **Stop shutting down your services just to inspect the database. Stop staring at hex dumps trying to guess what the data means.**
>
> `rocksdb-tui` connects via Secondary read-only mode — no locks, no risk, safe for production. It automatically decodes Protobuf, MessagePack, Molecule and other binary formats into readable JSON. Browse millions of keys with pagination, search by prefix, and refresh live data — all from your terminal.

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
  - **WASM Plugins** - Custom formats via WebAssembly plugins

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
# String keys and JSON values
[[column_families]]
name = "users"
key_schema = "string"      # preset: string, hex, u8le, u4be, etc.
value_format = "json"      # string, hex, json, msgpack, protobuf, molecule

# MessagePack values with hex keys
[[column_families]]
name = "sessions"
key_schema = "hex"
value_format = "msgpack"

# Protobuf values (dynamic decoding, no protoc needed)
[[column_families]]
name = "orders"
key_schema = "string"
value_format = "protobuf"
proto_file = "protos/order.proto"
proto_message = "Order"
proto_includes = ["protos/"]  # optional: additional import paths

# Molecule values (CKB format)
[[column_families]]
name = "accounts"
key_schema = "string"
value_format = "molecule"
mol_file = "schemas/types.mol"
mol_type = "Account"

# Composite binary keys using Kaitai-style schema
[[column_families]]
name = "blocks"
key_schema = """
seq:
  - id: block_num
    type: u8le
  - id: tx_index
    type: u4le
"""
value_format = "json"
```

### Key Schema

The `key_schema` field supports both presets (simple strings) and custom Kaitai-style YAML schemas for composite binary keys.

#### Presets

| Preset | Description |
|--------|-------------|
| `string` | UTF-8 string (default) |
| `hex` | Hexadecimal dump |
| `u1`, `u2le`, `u2be`, `u4le`, `u4be`, `u8le`, `u8be` | Unsigned integers (1/2/4/8 bytes, little/big endian) |
| `s1`, `s2le`, `s2be`, `s4le`, `s4be`, `s8le`, `s8be` | Signed integers |

#### Custom Schema

For composite keys (e.g., `block_number + tx_index`), use a Kaitai-style YAML schema:

```yaml
seq:
  - id: block_num    # field name
    type: u8le       # u64 little-endian
  - id: tx_index
    type: u4le       # u32 little-endian
```

**Supported field types:**

| Type | Description |
|------|-------------|
| `u1`, `u2le`, `u2be`, `u4le`, `u4be`, `u8le`, `u8be` | Unsigned integers |
| `s1`, `s2le`, `s2be`, `s4le`, `s4be`, `s8le`, `s8be` | Signed integers |
| `bytes` | Fixed-size bytes (requires `size` attribute) |
| `str` | Fixed-size UTF-8 string (requires `size` attribute) |
| `strz` | Null-terminated string |
| `vlq` | Variable-length quantity (unsigned) |

**Example with bytes and enum:**

```yaml
enums:
  tx_type:
    0: coinbase
    1: transfer
    2: contract
seq:
  - id: block_hash
    type: bytes
    size: 32
  - id: tx_type
    type: u1
    enum: tx_type
  - id: tx_index
    type: u4le
```

Keys display as: `block_hash: 0x1a2b3c..., tx_type: transfer, tx_index: 42`

#### External Schema File

For complex schemas, use a separate file:

```toml
[[column_families]]
name = "blocks"
key_schema_file = "schemas/block_key.yaml"
value_format = "json"
```

### Value Schema

For binary structured values (not covered by standard formats like JSON/Protobuf/MessagePack), use `value_schema` with the same Kaitai-style DSL:

```toml
[[column_families]]
name = "metrics"
key_schema = "string"
value_schema = """
seq:
  - id: timestamp
    type: u8le
  - id: value
    type: u8le
  - id: flags
    type: u4le
"""
```

Values display as: `timestamp: 1699123456, value: 42, flags: 1`

The `value_schema` field supports:
- All the same presets as `key_schema` (`string`, `hex`, `u8le`, etc.)
- Full Kaitai-style YAML schemas with composite fields
- External files via `value_schema_file`

**Note:** `value_schema` takes precedence over `value_format`. Use `value_format` for standard formats (JSON, Protobuf, etc.) and `value_schema` for custom binary structures.

### WASM Plugins

For custom binary formats not covered by built-in parsers, rocksdb-tui supports WASM plugins. Plugins can decode application-specific formats (like bincode-serialized Rust structs) into readable JSON.

#### Configuration

```toml
[plugins]
wasm = [
    "~/.config/rocksdb-tui/plugins/fiber-parser.wasm",
    "/absolute/path/to/another-plugin.wasm"
]

[[column_families]]
name = "default"
key_schema = "hex"
value_format = "fiber"  # Plugin receives key + value, routes by key prefix
```

The `value_format` can be any string. If it doesn't match a built-in format (string, hex, json, msgpack, protobuf, molecule), rocksdb-tui looks for a plugin that handles that format.

#### Plugin ABI

Plugins are WebAssembly modules that export these functions:

| Export | Signature | Description |
|--------|-----------|-------------|
| `memory` | Memory | Linear memory for data exchange |
| `alloc` | `(size: u32) -> u32` | Allocate bytes, return pointer |
| `dealloc` | `(ptr: u32, len: u32)` | Free allocated bytes |
| `get_formats` | `() -> u64` | Return packed ptr+len to JSON array of format names |
| `parse` | `(fmt_ptr, fmt_len, key_ptr, key_len, val_ptr, val_len) -> u64` | Parse value using key for routing |

The packed return value encodes `(ptr << 32) | len`.

**Key-based routing:** The `parse` function receives both key and value. This allows plugins to route parsing based on key prefixes - useful for databases like Fiber Network that use a single column family with prefix bytes to distinguish value types.

#### Writing a Plugin

Use the `rocksdb-tui-plugin-sdk` crate (see `crates/rocksdb-tui-plugin-sdk/`):

```rust
use rocksdb_tui_plugin_sdk::*;

// Parser function receives format, key, and value
fn my_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    // Route by key prefix for single-CF databases
    if format == "fiber" {
        return match key.first() {
            Some(0x00) => parse_channel_state(value),
            Some(0x01) => parse_payment_session(value),
            _ => Ok("{}".to_string()),
        };
    }
    
    // Or parse by format name
    match format {
        "myapp.User" => {
            let user: User = bincode::deserialize(value).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&user).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown format: {}", format))
    }
}

export_plugin! {
    formats: ["fiber", "myapp.User"],
    parse: my_parser
}
```

Build with:
```bash
cargo build --target wasm32-unknown-unknown --release
```

#### Example: Fiber Network

See `examples/fiber/` for a complete example using a WASM plugin to decode Fiber Network's bincode-serialized data with prefix-based routing.

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
