//! Demo Types Library
//!
//! This crate simulates a third-party library that defines
//! data structures used by a RocksDB-based application. The types are serialized
//! using bincode and stored in RocksDB.
//!
//! In real-world scenarios, this would be a separate crate published by the
//! application developers.

use serde::{Deserialize, Serialize};

/// Key prefix constants for key-based routing in single-CF mode
pub mod prefix {
    pub const PRODUCT: u8 = 0x00;
    pub const CUSTOMER: u8 = 0x01;
    pub const TRANSACTION: u8 = 0x02;
}

/// Product record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Product {
    pub id: u32,
    pub name: String,
    pub price: f64,
    pub in_stock: bool,
}

impl Product {
    pub fn new(id: u32, name: impl Into<String>, price: f64, in_stock: bool) -> Self {
        Self {
            id,
            name: name.into(),
            price,
            in_stock,
        }
    }

    /// Encode to bincode bytes
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize Product")
    }

    /// Decode from bincode bytes
    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Create key for single-CF mode (with prefix)
    pub fn key_with_prefix(&self) -> Vec<u8> {
        let mut key = vec![prefix::PRODUCT];
        key.extend_from_slice(&self.id.to_le_bytes());
        key
    }

    /// Create key for multi-CF mode (without prefix)
    pub fn key(&self) -> Vec<u8> {
        self.id.to_le_bytes().to_vec()
    }
}

/// Customer tier levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum CustomerTier {
    Bronze = 0,
    Silver = 1,
    Gold = 2,
    Platinum = 3,
}

impl CustomerTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            CustomerTier::Bronze => "Bronze",
            CustomerTier::Silver => "Silver",
            CustomerTier::Gold => "Gold",
            CustomerTier::Platinum => "Platinum",
        }
    }
}

/// Customer record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Customer {
    pub id: u32,
    pub name: String,
    pub email: String,
    pub tier: CustomerTier,
}

impl Customer {
    pub fn new(
        id: u32,
        name: impl Into<String>,
        email: impl Into<String>,
        tier: CustomerTier,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            email: email.into(),
            tier,
        }
    }

    /// Encode to bincode bytes
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize Customer")
    }

    /// Decode from bincode bytes
    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Create key for single-CF mode (with prefix)
    pub fn key_with_prefix(&self) -> Vec<u8> {
        let mut key = vec![prefix::CUSTOMER];
        key.extend_from_slice(&self.id.to_le_bytes());
        key
    }

    /// Create key for multi-CF mode (without prefix)
    pub fn key(&self) -> Vec<u8> {
        self.id.to_le_bytes().to_vec()
    }
}

/// Transaction record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub id: u32,
    pub customer_id: u32,
    pub product_id: u32,
    pub quantity: u16,
    pub timestamp: u64,
}

impl Transaction {
    pub fn new(id: u32, customer_id: u32, product_id: u32, quantity: u16, timestamp: u64) -> Self {
        Self {
            id,
            customer_id,
            product_id,
            quantity,
            timestamp,
        }
    }

    /// Encode to bincode bytes
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize Transaction")
    }

    /// Decode from bincode bytes
    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Create key for single-CF mode (with prefix)
    pub fn key_with_prefix(&self) -> Vec<u8> {
        let mut key = vec![prefix::TRANSACTION];
        key.extend_from_slice(&self.id.to_le_bytes());
        key
    }

    /// Create key for multi-CF mode (without prefix)
    pub fn key(&self) -> Vec<u8> {
        self.id.to_le_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_roundtrip() {
        let product = Product::new(1, "Widget", 9.99, true);
        let encoded = product.encode();
        let decoded = Product::decode(&encoded).unwrap();
        assert_eq!(product, decoded);
    }

    #[test]
    fn test_customer_roundtrip() {
        let customer = Customer::new(101, "Alice", "alice@example.com", CustomerTier::Gold);
        let encoded = customer.encode();
        let decoded = Customer::decode(&encoded).unwrap();
        assert_eq!(customer, decoded);
    }

    #[test]
    fn test_transaction_roundtrip() {
        let tx = Transaction::new(1001, 101, 1, 2, 1704067200);
        let encoded = tx.encode();
        let decoded = Transaction::decode(&encoded).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_key_with_prefix() {
        let product = Product::new(1, "Widget", 9.99, true);
        let key = product.key_with_prefix();
        assert_eq!(key[0], prefix::PRODUCT);
        assert_eq!(&key[1..], &1u32.to_le_bytes());
    }
}
