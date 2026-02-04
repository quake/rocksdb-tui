//! Demo WASM plugin for rocksdb-tui
//!
//! This plugin demonstrates two parsing modes:
//!
//! ## Mode 1: Key-based routing (single CF)
//! - Format: `demo`
//! - Uses key prefix byte to determine value type
//! - Key prefix: 0x00=Product, 0x01=Customer, 0x02=Transaction
//!
//! ## Mode 2: Format-based (multi CF)
//! - Format: `demo.Product`, `demo.Customer`, `demo.Transaction`
//! - Each CF has its own format, no key prefix needed
//!
//! Both modes use bincode serialization from the `demo-types` crate.

use demo_types::{prefix, Customer, Product, Transaction};
use rocksdb_tui_plugin_sdk::*;

fn parse_product(value: &[u8]) -> Result<String, String> {
    let product = Product::decode(value).map_err(|e| format!("Failed to decode Product: {}", e))?;
    serde_json::to_string_pretty(&product).map_err(|e| e.to_string())
}

fn parse_customer(value: &[u8]) -> Result<String, String> {
    let customer =
        Customer::decode(value).map_err(|e| format!("Failed to decode Customer: {}", e))?;
    serde_json::to_string_pretty(&customer).map_err(|e| e.to_string())
}

fn parse_transaction(value: &[u8]) -> Result<String, String> {
    let tx =
        Transaction::decode(value).map_err(|e| format!("Failed to decode Transaction: {}", e))?;
    serde_json::to_string_pretty(&tx).map_err(|e| e.to_string())
}

fn demo_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    match format {
        // Mode 1: Key-based routing (single CF with prefix)
        "demo" => match key.first() {
            Some(&p) if p == prefix::PRODUCT => parse_product(value),
            Some(&p) if p == prefix::CUSTOMER => parse_customer(value),
            Some(&p) if p == prefix::TRANSACTION => parse_transaction(value),
            Some(p) => Err(format!("Unknown key prefix: 0x{:02x}", p)),
            None => Err("Empty key".to_string()),
        },

        // Mode 2: Format-based (multi CF, direct type mapping)
        "demo.Product" => parse_product(value),
        "demo.Customer" => parse_customer(value),
        "demo.Transaction" => parse_transaction(value),

        _ => Err(format!("Unknown format: {}", format)),
    }
}

/// Parse key into human-readable JSON format
fn demo_key_parser(format: &str, key: &[u8]) -> Result<String, String> {
    match format {
        // Mode 1: Key format is [prefix: u8][id: u32 LE]
        "demo" => {
            if key.is_empty() {
                return Err("Empty key".to_string());
            }
            let prefix_byte = key[0];
            let type_name = match prefix_byte {
                p if p == prefix::PRODUCT => "Product",
                p if p == prefix::CUSTOMER => "Customer",
                p if p == prefix::TRANSACTION => "Transaction",
                p => return Err(format!("Unknown key prefix: 0x{:02x}", p)),
            };

            // Parse ID from remaining bytes (u32 LE)
            let id = if key.len() >= 5 {
                u32::from_le_bytes([key[1], key[2], key[3], key[4]])
            } else {
                return Err(format!(
                    "Key too short: expected 5 bytes, got {}",
                    key.len()
                ));
            };

            Ok(serde_json::json!({
                "type": type_name,
                "id": id
            })
            .to_string())
        }

        // Mode 2: Key format is just [id: u32 LE]
        "demo.Product" | "demo.Customer" | "demo.Transaction" => {
            if key.len() < 4 {
                return Err(format!(
                    "Key too short: expected 4 bytes, got {}",
                    key.len()
                ));
            }
            let id = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
            Ok(serde_json::json!({ "id": id }).to_string())
        }

        _ => Err(format!("Unknown format: {}", format)),
    }
}

export_plugin! {
    formats: ["demo", "demo.Product", "demo.Customer", "demo.Transaction"],
    parse: demo_parser,
    parse_key: demo_key_parser
}
