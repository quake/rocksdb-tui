//! Fiber Network WASM Plugin for rocksdb-tui
//!
//! This plugin decodes Fiber Network's bincode-serialized data stored in RocksDB.
//! Fiber uses a single column family with 1-byte key prefixes to distinguish data types.
//!
//! ## Key Prefixes (from Fiber's schema.rs)
//!
//! | Prefix | Key                  | Value                       |
//! |--------|----------------------|-----------------------------|
//! | 0      | Hash256              | ChannelActorState           |
//! | 16     | PeerId               | PersistentNetworkActorState |
//! | 32     | Hash256              | CkbInvoice                  |
//! | 33     | Payment_hash         | CkbInvoice Preimage         |
//! | 34     | Payment_hash         | CkbInvoice Status           |
//! | 64     | PeerId + Hash256     | ChannelState                |
//! | 65     | OutPoint             | ChannelId                   |
//! | 96     | Cursor               | BroadcastMessage            |
//! | 97     | BroadcastMessageID   | u64                         |
//! | 192    | Hash256              | PaymentSession              |
//! | 193    | OutPoint + Direction | TimedResult                 |
//! | 194    | Hash256              | PaymentCustomRecords        |

use fiber_types::{prefix, ChannelState, CkbInvoiceStatus, Hash256, PaymentStatus};
use rocksdb_tui_plugin_sdk::*;

fn decode_and_json<'a, T: serde::Deserialize<'a> + serde::Serialize>(
    value: &'a [u8],
    type_name: &str,
) -> Result<String, String> {
    let decoded: T = bincode::deserialize(value)
        .map_err(|e| format!("Failed to decode {}: {}", type_name, e))?;
    serde_json::to_string_pretty(&decoded).map_err(|e| format!("JSON error: {}", e))
}

fn parse_hash256(value: &[u8], label: &str) -> Result<String, String> {
    let hash: Hash256 =
        bincode::deserialize(value).map_err(|e| format!("Failed to decode {}: {}", label, e))?;
    Ok(serde_json::json!({ label: hash.to_string() }).to_string())
}

fn parse_channel_state(value: &[u8]) -> Result<String, String> {
    decode_and_json::<ChannelState>(value, "ChannelState")
}

fn parse_invoice_status(value: &[u8]) -> Result<String, String> {
    decode_and_json::<CkbInvoiceStatus>(value, "CkbInvoiceStatus")
}

fn parse_payment_status(value: &[u8]) -> Result<String, String> {
    decode_and_json::<PaymentStatus>(value, "PaymentStatus")
}

fn parse_timestamp(value: &[u8]) -> Result<String, String> {
    if value.len() == 8 {
        let timestamp = u64::from_be_bytes(value.try_into().unwrap());
        Ok(serde_json::json!({ "timestamp": timestamp }).to_string())
    } else {
        Err(format!(
            "Invalid timestamp length: {} (expected 8)",
            value.len()
        ))
    }
}

fn fiber_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    if format != "fiber" {
        return Err(format!("Unknown format: {}", format));
    }

    // Route by key prefix
    match key.first() {
        Some(&prefix::CHANNEL_ACTOR_STATE) => {
            // ChannelActorState is complex, show summary
            Ok(serde_json::json!({
                "type": "ChannelActorState",
                "key_hash": hex::encode(&key[1..]),
                "value_size": value.len(),
                "hint": "Complex type - full decode requires fnn crate"
            })
            .to_string())
        }
        Some(&prefix::PEER_ID_NETWORK_ACTOR_STATE) => Ok(serde_json::json!({
            "type": "PersistentNetworkActorState",
            "peer_id": hex::encode(&key[1..]),
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::CKB_INVOICE) => Ok(serde_json::json!({
            "type": "CkbInvoice",
            "payment_hash": hex::encode(&key[1..]),
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::PREIMAGE) => parse_hash256(value, "preimage"),
        Some(&prefix::CKB_INVOICE_STATUS) => parse_invoice_status(value),
        Some(&prefix::PEER_ID_CHANNEL_ID) => parse_channel_state(value),
        Some(&prefix::CHANNEL_OUTPOINT_CHANNEL_ID) => parse_hash256(value, "channel_id"),
        Some(&prefix::BROADCAST_MESSAGE) => Ok(serde_json::json!({
            "type": "BroadcastMessage",
            "cursor": hex::encode(&key[1..]),
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::BROADCAST_MESSAGE_TIMESTAMP) => parse_timestamp(value),
        Some(&prefix::PAYMENT_SESSION) => Ok(serde_json::json!({
            "type": "PaymentSession",
            "payment_hash": hex::encode(&key[1..]),
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::PAYMENT_HISTORY_TIMED_RESULT) => Ok(serde_json::json!({
            "type": "PaymentHistoryTimedResult",
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::PAYMENT_CUSTOM_RECORD) => Ok(serde_json::json!({
            "type": "PaymentCustomRecords",
            "payment_hash": hex::encode(&key[1..]),
            "value_size": value.len()
        })
        .to_string()),
        Some(&prefix::ATTEMPT) => Ok(serde_json::json!({
            "type": "Attempt",
            "value_size": value.len()
        })
        .to_string()),
        Some(p) => {
            // Unknown prefix - show as hex with prefix info
            Ok(serde_json::json!({
                "unknown_prefix": format!("0x{:02x}", p),
                "key_hex": hex::encode(key),
                "value_hex": hex::encode(value)
            })
            .to_string())
        }
        None => Err("Empty key".to_string()),
    }
}

// Simple hex encoding
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            result.push(HEX_CHARS[(b >> 4) as usize] as char);
            result.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        result
    }
}

export_plugin! {
    formats: ["fiber"],
    parse: fiber_parser
}
