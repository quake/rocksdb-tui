# Fiber Network RocksDB Configuration

This directory contains the configuration files for browsing [Fiber Network](https://github.com/nervosnetwork/fiber) RocksDB databases with `rocksdb-tui`.

## Files

- `config.toml` - Parser configuration for Fiber Network database
- `fiber_parser.wasm` - Pre-built WASM plugin for decoding bincode values
- `fiber-types/` - Simplified Fiber type definitions
- `fiber-parser/` - WASM plugin source code

## Usage

```bash
# Browse Fiber Network database
rocksdb-tui --db /path/to/fiber/store --config examples/fiber/config.toml
```

## Key Prefixes

Fiber Network uses a single column family (`default`) with 1-byte key prefixes:

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

## Building the Plugin

```bash
cd examples/fiber/fiber-parser
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/fiber_parser.wasm ../
```

## Notes

- All Fiber data types use bincode serialization
- The plugin routes parsing based on key prefix byte
- Complex types show metadata; simple types are fully deserialized
