# Plugin Demo

This directory contains a demo WASM plugin for `rocksdb-tui`, demonstrating how to build custom value parsers.

## Files

- `test-config.toml` - Configuration file for testing with the demo database
- `demo_parser.wasm` - Pre-built WASM plugin
- `demo-types/` - Type definitions with bincode serialization
- `demo-parser/` - WASM plugin source code

## Usage

```bash
# Create the test database
cargo run --example create_test_db

# Browse with the demo plugin
rocksdb-tui --db /tmp/rocksdb-tui-test-db --config examples/plugin-demo/test-config.toml
```

## Plugin Modes

The demo plugin supports two parsing modes:

### Mode 1: Key-prefix Routing (Single CF)

All data in one column family, key prefix determines value type:

| Prefix | Type        |
|--------|-------------|
| 0x00   | Product     |
| 0x01   | Customer    |
| 0x02   | Transaction |

```toml
[[column_families]]
name = "demo"
key_schema = "hex"
value_format = "demo"
```

### Mode 2: Format-based Routing (Multi CF)

Each column family has its own format:

```toml
[[column_families]]
name = "products"
value_format = "demo.Product"

[[column_families]]
name = "customers"
value_format = "demo.Customer"

[[column_families]]
name = "transactions"
value_format = "demo.Transaction"
```

## Building the Plugin

```bash
cd examples/plugin-demo/demo-parser
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/demo_parser.wasm ../
```

## Notes

- All demo types use bincode serialization
- The plugin demonstrates both single-CF and multi-CF patterns
- See `demo-types/src/lib.rs` for type definitions
