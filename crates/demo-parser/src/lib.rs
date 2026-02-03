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

export_plugin! {
    formats: ["demo", "demo.Product", "demo.Customer", "demo.Transaction"],
    parse: demo_parser
}
