//! Demo WASM plugin for rocksdb-tui
//!
//! This plugin demonstrates key-based routing for parsing different value types
//! within a single column family. The key prefix byte determines the value type:
//!
//! - 0x00: Product (id: u32, name, price: f64, in_stock: bool)
//! - 0x01: Customer (id: u32, name, email, tier: u8)
//! - 0x02: Transaction (id: u32, customer_id: u32, product_id: u32, quantity: u16, timestamp: u64)

use rocksdb_tui_plugin_sdk::*;
use serde::Serialize;

/// Product record - prefix 0x00
/// Binary format: id (4 bytes u32 LE) | name_len (1 byte) | name (variable) | price (8 bytes f64 LE) | in_stock (1 byte bool)
#[derive(Serialize)]
struct Product {
    id: u32,
    name: String,
    price: f64,
    in_stock: bool,
}

/// Customer record - prefix 0x01
/// Binary format: id (4 bytes u32 LE) | name_len (1 byte) | name | email_len (1 byte) | email | tier (1 byte)
#[derive(Serialize)]
struct Customer {
    id: u32,
    name: String,
    email: String,
    tier: String,
}

/// Transaction record - prefix 0x02
/// Binary format: id (4 bytes u32 LE) | customer_id (4 bytes) | product_id (4 bytes) | quantity (2 bytes u16 LE) | timestamp (8 bytes u64 LE)
#[derive(Serialize)]
struct Transaction {
    id: u32,
    customer_id: u32,
    product_id: u32,
    quantity: u16,
    timestamp: u64,
}

fn parse_product(value: &[u8]) -> Result<String, String> {
    if value.len() < 6 {
        return Err("Product value too short".to_string());
    }

    let id = u32::from_le_bytes(value[0..4].try_into().unwrap());
    let name_len = value[4] as usize;

    if value.len() < 5 + name_len + 9 {
        return Err("Product value truncated".to_string());
    }

    let name = String::from_utf8_lossy(&value[5..5 + name_len]).to_string();
    let price_offset = 5 + name_len;
    let price = f64::from_le_bytes(value[price_offset..price_offset + 8].try_into().unwrap());
    let in_stock = value[price_offset + 8] != 0;

    let product = Product {
        id,
        name,
        price,
        in_stock,
    };
    serde_json::to_string_pretty(&product).map_err(|e| e.to_string())
}

fn parse_customer(value: &[u8]) -> Result<String, String> {
    if value.len() < 7 {
        return Err("Customer value too short".to_string());
    }

    let id = u32::from_le_bytes(value[0..4].try_into().unwrap());
    let name_len = value[4] as usize;

    if value.len() < 5 + name_len + 2 {
        return Err("Customer value truncated (name)".to_string());
    }

    let name = String::from_utf8_lossy(&value[5..5 + name_len]).to_string();
    let email_offset = 5 + name_len;
    let email_len = value[email_offset] as usize;

    if value.len() < email_offset + 1 + email_len + 1 {
        return Err("Customer value truncated (email)".to_string());
    }

    let email =
        String::from_utf8_lossy(&value[email_offset + 1..email_offset + 1 + email_len]).to_string();
    let tier_byte = value[email_offset + 1 + email_len];
    let tier = match tier_byte {
        0 => "Bronze",
        1 => "Silver",
        2 => "Gold",
        3 => "Platinum",
        _ => "Unknown",
    }
    .to_string();

    let customer = Customer {
        id,
        name,
        email,
        tier,
    };
    serde_json::to_string_pretty(&customer).map_err(|e| e.to_string())
}

fn parse_transaction(value: &[u8]) -> Result<String, String> {
    if value.len() < 22 {
        return Err(format!(
            "Transaction value too short: {} bytes",
            value.len()
        ));
    }

    let id = u32::from_le_bytes(value[0..4].try_into().unwrap());
    let customer_id = u32::from_le_bytes(value[4..8].try_into().unwrap());
    let product_id = u32::from_le_bytes(value[8..12].try_into().unwrap());
    let quantity = u16::from_le_bytes(value[12..14].try_into().unwrap());
    let timestamp = u64::from_le_bytes(value[14..22].try_into().unwrap());

    let tx = Transaction {
        id,
        customer_id,
        product_id,
        quantity,
        timestamp,
    };
    serde_json::to_string_pretty(&tx).map_err(|e| e.to_string())
}

fn demo_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
    if format != "demo" {
        return Err(format!("Unknown format: {}", format));
    }

    // Key-based routing: first byte of key determines value type
    match key.first() {
        Some(0x00) => parse_product(value),
        Some(0x01) => parse_customer(value),
        Some(0x02) => parse_transaction(value),
        Some(prefix) => Err(format!("Unknown key prefix: 0x{:02x}", prefix)),
        None => Err("Empty key".to_string()),
    }
}

export_plugin! {
    formats: ["demo"],
    parse: demo_parser
}
