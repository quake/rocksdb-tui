# Fiber Network Example

This example shows how to configure rocksdb-tui for browsing a [Fiber Network](https://github.com/nervosnetwork/fiber) RocksDB database.

## Data Model

Fiber Network uses a **single column family** (`default`) with **1-byte key prefixes** to distinguish different data types:

```
Key format: [prefix: 1 byte][type-specific key data]
```

The WASM plugin inspects `key[0]` to determine the value type and deserialize accordingly.

## Key Prefixes

| Prefix | Data Type | Key Format |
|--------|-----------|------------|
| 0x00 | ChannelActorState | peer_id (32) + channel_id (32) |
| 0x01 | PaymentSession | payment_hash (32) |
| 0x02 | PeerInfo | peer_id (32) |
| 0x03 | ChannelAnnouncement | tx_hash (32) + index (4) |
| 0x04 | NodeAnnouncement | node_id (33) |
| 0x05 | Invoice | payment_hash (32) |
| 0x06 | WatchtowerData | channel_id (32) |
| 0x07 | GraphNode | node_id (33) |
| 0x08 | GraphEdge | short_channel_id (8) |

## Prerequisites

1. Build the `fiber-parser` WASM plugin
2. Copy the compiled `.wasm` file to `~/.config/rocksdb-tui/plugins/fiber-parser.wasm`

## Usage

```bash
rocksdb-tui --db /path/to/fiber/store --config examples/fiber/config.toml
```

## Plugin Implementation

The plugin receives both key and value, routing by key prefix:

```rust
use rocksdb_tui_plugin_sdk::*;

fn parse_fiber(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    if format != "fiber" {
        return Err("Unknown format".to_string());
    }
    
    match key.first() {
        Some(0x00) => parse_channel_state(value),
        Some(0x01) => parse_payment_session(value),
        Some(0x02) => parse_peer_info(value),
        // ... etc
        _ => Ok(format!("{{\"hex\": \"{}\"}}", hex::encode(value))),
    }
}

export_plugin! {
    formats: ["fiber"],
    parse: parse_fiber
}
```

All Fiber data types are bincode-serialized Rust structs that get decoded to JSON for display.
