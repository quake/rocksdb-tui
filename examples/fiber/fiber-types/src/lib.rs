//! Simplified Fiber Network types for rocksdb-tui WASM plugin
//!
//! These types are simplified versions of the actual Fiber types, designed to
//! deserialize bincode-encoded data from Fiber's RocksDB and display it as JSON.
//!
//! Note: Some complex nested types use `serde_json::Value` as a catch-all to
//! avoid requiring the full dependency tree of the fnn crate.

use serde::{Deserialize, Serialize};

/// Key prefixes from Fiber's schema.rs
pub mod prefix {
    pub const CHANNEL_ACTOR_STATE: u8 = 0;
    pub const PEER_ID_NETWORK_ACTOR_STATE: u8 = 16;
    pub const CKB_INVOICE: u8 = 32;
    pub const PREIMAGE: u8 = 33;
    pub const CKB_INVOICE_STATUS: u8 = 34;
    pub const PEER_ID_CHANNEL_ID: u8 = 64;
    pub const CHANNEL_OUTPOINT_CHANNEL_ID: u8 = 65;
    pub const BROADCAST_MESSAGE: u8 = 96;
    pub const BROADCAST_MESSAGE_TIMESTAMP: u8 = 97;
    pub const PAYMENT_SESSION: u8 = 192;
    pub const PAYMENT_HISTORY_TIMED_RESULT: u8 = 193;
    pub const PAYMENT_CUSTOM_RECORD: u8 = 194;
    pub const ATTEMPT: u8 = 195;
}

/// 32-byte hash, displayed as hex
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash256(#[serde(with = "hex_bytes")] pub [u8; 32]);

impl std::fmt::Display for Hash256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in &self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

/// Helper module for hex serialization of byte arrays
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        let hex_string: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        hex_string.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        // bincode stores raw bytes, not hex strings
        <[u8; 32]>::deserialize(d)
    }
}

/// Channel state enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelState {
    NegotiatingFunding(NegotiatingFundingFlags),
    CollaboratingFundingTx(CollaboratingFundingTxFlags),
    SigningCommitment(SigningCommitmentFlags),
    AwaitingTxSignatures(AwaitingTxSignaturesFlags),
    AwaitingChannelReady(AwaitingChannelReadyFlags),
    ChannelReady,
    ShuttingDown(ShuttingDownFlags),
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiatingFundingFlags {
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaboratingFundingTxFlags {
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningCommitmentFlags {
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitingTxSignaturesFlags {
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitingChannelReadyFlags {
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuttingDownFlags {
    pub flags: u32,
}

/// Invoice status
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CkbInvoiceStatus {
    Open,
    Cancelled,
    Expired,
    Received,
    Paid,
}

/// Payment session status  
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PaymentStatus {
    Inflight,
    Success,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash256_display() {
        let hash = Hash256([0u8; 32]);
        assert_eq!(
            hash.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
