//! Fiber Network WASM Plugin for rocksdb-tui
//!
//! This plugin decodes Fiber Network's bincode-serialized data stored in RocksDB.
//! Fiber uses a single column family with 1-byte key prefixes to distinguish data types.
//!
//! ## Key Prefixes
//!
//! | Prefix | Data Type |
//! |--------|-----------|
//! | 0x00 | ChannelState |
//! | 0x01 | PaymentSession |
//! | 0x02 | PeerInfo |
//! | 0x05 | Invoice |

use fiber_types::{prefix, ChannelState, Invoice, PaymentSession, PeerInfo};
use rocksdb_tui_plugin_sdk::*;

fn parse_channel_state(value: &[u8]) -> Result<String, String> {
    let state =
        ChannelState::decode(value).map_err(|e| format!("Failed to decode ChannelState: {}", e))?;
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

fn parse_payment_session(value: &[u8]) -> Result<String, String> {
    let session = PaymentSession::decode(value)
        .map_err(|e| format!("Failed to decode PaymentSession: {}", e))?;
    serde_json::to_string_pretty(&session).map_err(|e| e.to_string())
}

fn parse_peer_info(value: &[u8]) -> Result<String, String> {
    let info = PeerInfo::decode(value).map_err(|e| format!("Failed to decode PeerInfo: {}", e))?;
    serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
}

fn parse_invoice(value: &[u8]) -> Result<String, String> {
    let invoice = Invoice::decode(value).map_err(|e| format!("Failed to decode Invoice: {}", e))?;
    serde_json::to_string_pretty(&invoice).map_err(|e| e.to_string())
}

fn fiber_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    if format != "fiber" {
        return Err(format!("Unknown format: {}", format));
    }

    // Route by key prefix
    match key.first() {
        Some(&p) if p == prefix::CHANNEL_STATE => parse_channel_state(value),
        Some(&p) if p == prefix::PAYMENT_SESSION => parse_payment_session(value),
        Some(&p) if p == prefix::PEER_INFO => parse_peer_info(value),
        Some(&p) if p == prefix::INVOICE => parse_invoice(value),
        Some(p) => {
            // Unknown prefix - show as hex
            Ok(serde_json::json!({
                "unknown_prefix": format!("0x{:02x}", p),
                "value_hex": hex::encode(value)
            })
            .to_string())
        }
        None => Err("Empty key".to_string()),
    }
}

// Simple hex encoding for unknown prefixes
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
