# Fiber Network Example

This example shows how to configure rocksdb-tui for browsing a [Fiber Network](https://github.com/nervosnetwork/fiber) RocksDB database.

## Overview

Fiber Network uses a **single column family** (`default`) with **1-byte key prefixes** to distinguish different data types. The WASM plugin inspects `key[0]` to determine the value type and deserialize accordingly using **bincode**.

## Key Prefixes (from Fiber's schema.rs)

| Prefix | Key                  | Value                       |
|--------|----------------------|-----------------------------|
| 0      | Hash256              | ChannelActorState           |
| 16     | PeerId               | PersistentNetworkActorState |
| 32     | Hash256              | CkbInvoice                  |
| 33     | Payment_hash         | CkbInvoice Preimage         |
| 34     | Payment_hash         | CkbInvoice Status           |
| 64     | PeerId + Hash256     | ChannelState                |
| 65     | OutPoint             | ChannelId                   |
| 96     | Cursor               | BroadcastMessage            |
| 97     | BroadcastMessageID   | u64                         |
| 192    | Hash256              | PaymentSession              |
| 193    | OutPoint + Direction | TimedResult                 |
| 194    | Hash256              | PaymentCustomRecords        |

## Project Structure

```
examples/fiber/
├── README.md
├── config.toml          # rocksdb-tui configuration
├── fiber_parser.wasm    # Pre-built WASM plugin
├── fiber-types/         # Simplified Fiber type definitions
│   ├── Cargo.toml
│   └── src/lib.rs
└── fiber-parser/        # Plugin source code
    ├── Cargo.toml
    └── src/lib.rs
```

## Build

```bash
# Build the WASM plugin
cd examples/fiber/fiber-parser
cargo build --release --target wasm32-unknown-unknown

# Copy to examples directory
cp target/wasm32-unknown-unknown/release/fiber_parser.wasm ../
```

## Usage

```bash
# With a real Fiber Network database
rocksdb-tui --db /path/to/fiber/store --config examples/fiber/config.toml
```

## Plugin Implementation

The plugin uses simplified type definitions from `fiber-types` crate and routes by key prefix:

```rust
use fiber_types::{prefix, ChannelState, CkbInvoiceStatus, Hash256};
use rocksdb_tui_plugin_sdk::*;

fn fiber_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    match key.first() {
        Some(&prefix::CHANNEL_ACTOR_STATE) => {
            // Complex type - show metadata
            Ok(json!({
                "type": "ChannelActorState",
                "key_hash": hex::encode(&key[1..]),
                "value_size": value.len()
            }).to_string())
        }
        Some(&prefix::CKB_INVOICE_STATUS) => {
            let status: CkbInvoiceStatus = bincode::deserialize(value)?;
            serde_json::to_string_pretty(&status)
        }
        Some(&prefix::PEER_ID_CHANNEL_ID) => {
            let state: ChannelState = bincode::deserialize(value)?;
            serde_json::to_string_pretty(&state)
        }
        // ... other prefixes
        _ => Ok(json!({"hex": hex::encode(value)}).to_string()),
    }
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

## Notes

The `fiber-types` crate contains simplified versions of Fiber's data types. For complex types like `ChannelActorState` that have deep dependency trees, the plugin shows metadata (type name, key hash, value size) rather than full deserialization. Simple types like `ChannelState`, `CkbInvoiceStatus`, and `Hash256` are fully deserialized.

To decode all Fiber types with full fidelity, the plugin would need to import the `fnn` crate directly, but this requires `wasm-bindgen` which is incompatible with wasmtime. A future version could use a shared memory approach or RPC-style communication with the host.
