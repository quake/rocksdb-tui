# Fiber Network Example

This example shows how to configure rocksdb-tui for browsing a [Fiber Network](https://github.com/nervosnetwork/fiber) RocksDB database.

## Prerequisites

1. Build the `fiber-parser` WASM plugin from `crates/fiber-parser/`
2. Copy the compiled `.wasm` file to `~/.config/rocksdb-tui/plugins/fiber-parser.wasm`

## Usage

```bash
rocksdb-tui --db /path/to/fiber/store --config examples/fiber/config.toml
```

## Column Families

The config defines parsers for common Fiber Network column families:

| Column Family | Key Format | Value Format |
|---------------|------------|--------------|
| channel_actor_state | peer_id + channel_id | fiber.ChannelActorState |
| payment_session | payment_hash | fiber.PaymentSession |
| peer_info | peer_id | fiber.PeerInfo |
| channel_announcement | tx_hash + index | fiber.ChannelAnnouncement |
| node_announcement | node_id | fiber.NodeAnnouncement |
| invoice | payment_hash | fiber.Invoice |
| watchtower | channel_id | fiber.WatchtowerData |
| graph_node | node_id | fiber.GraphNode |
| graph_edge | short_channel_id | fiber.GraphEdge |

## Plugin Formats

The `fiber-parser.wasm` plugin provides these format handlers:

- `fiber.ChannelActorState` - Payment channel state
- `fiber.PaymentSession` - Active payment session
- `fiber.PeerInfo` - Peer connection info
- `fiber.ChannelAnnouncement` - Public channel announcement
- `fiber.NodeAnnouncement` - Public node announcement  
- `fiber.Invoice` - Payment invoice
- `fiber.WatchtowerData` - Channel monitoring data
- `fiber.GraphNode` - Network graph node
- `fiber.GraphEdge` - Network graph edge

All formats are bincode-serialized Rust structs that get decoded to JSON for display.
