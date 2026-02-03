# Fiber Network Example

This example shows how to configure rocksdb-tui for browsing a [Fiber Network](https://github.com/nervosnetwork/fiber) RocksDB database.

## Overview

Fiber Network uses a **single column family** (`default`) with **1-byte key prefixes** to distinguish different data types. The WASM plugin inspects `key[0]` to determine the value type and deserialize accordingly.

## Key Prefixes

| Prefix | Data Type | Description |
|--------|-----------|-------------|
| 0x00 | ChannelState | Channel state between two peers |
| 0x01 | PaymentSession | In-flight payment tracking |
| 0x02 | PeerInfo | Connected peer information |
| 0x05 | Invoice | Payment invoice |

## Project Structure

```
crates/
├── fiber-types/     # Data type definitions (simulates fiber-types crate)
│   └── src/lib.rs   # ChannelState, PaymentSession, PeerInfo, Invoice
└── fiber-parser/    # WASM plugin
    └── src/lib.rs   # Key-prefix routing and bincode decoding
```

## Build

```bash
# Build the WASM plugin
cargo build --release --target wasm32-unknown-unknown -p fiber-parser

# Copy to examples directory
cp target/wasm32-unknown-unknown/release/fiber_parser.wasm examples/fiber/
```

## Usage

```bash
# With a real Fiber Network database
rocksdb-tui --db /path/to/fiber/store --config examples/fiber/config.toml
```

## Plugin Implementation

The plugin routes by key prefix:

```rust
use fiber_types::{prefix, ChannelState, PaymentSession, PeerInfo, Invoice};
use rocksdb_tui_plugin_sdk::*;

fn fiber_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    match key.first() {
        Some(&p) if p == prefix::CHANNEL_STATE => {
            let state = ChannelState::decode(value)?;
            serde_json::to_string_pretty(&state)
        }
        Some(&p) if p == prefix::PAYMENT_SESSION => {
            let session = PaymentSession::decode(value)?;
            serde_json::to_string_pretty(&session)
        }
        // ... etc
        _ => Ok(format!("{{\"hex\": \"{}\"}}", hex::encode(value))),
    }
}

export_plugin! {
    formats: ["fiber"],
    parse: fiber_parser
}
```

All Fiber data types use bincode serialization and are decoded to JSON for display.

## Configuration

```toml
[plugins]
wasm = ["examples/fiber/fiber_parser.wasm"]

[[column_families]]
name = "default"
key_schema = "hex"
value_format = "fiber"
```
