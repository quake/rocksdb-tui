//! Fiber Network Types
//!
//! This crate defines data structures that simulate Fiber Network's storage format.
//! In a real implementation, these would match the actual Fiber Network types.
//!
//! Fiber uses a single column family with 1-byte key prefixes to distinguish data types.

use serde::{Deserialize, Serialize};

/// Key prefix constants for Fiber Network data types
pub mod prefix {
    pub const CHANNEL_STATE: u8 = 0x00;
    pub const PAYMENT_SESSION: u8 = 0x01;
    pub const PEER_INFO: u8 = 0x02;
    pub const CHANNEL_ANNOUNCEMENT: u8 = 0x03;
    pub const NODE_ANNOUNCEMENT: u8 = 0x04;
    pub const INVOICE: u8 = 0x05;
}

/// 32-byte hash type (simplified)
pub type Hash256 = [u8; 32];

/// 33-byte public key type (simplified)
pub type PublicKey = [u8; 33];

/// Channel state between two peers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelState {
    pub channel_id: Hash256,
    pub peer_id: Hash256,
    pub local_balance: u128,
    pub remote_balance: u128,
    pub status: ChannelStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChannelStatus {
    Opening,
    Open,
    Closing,
    Closed,
    ForceClosing,
}

impl ChannelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelStatus::Opening => "Opening",
            ChannelStatus::Open => "Open",
            ChannelStatus::Closing => "Closing",
            ChannelStatus::Closed => "Closed",
            ChannelStatus::ForceClosing => "ForceClosing",
        }
    }
}

impl ChannelState {
    pub fn new(
        channel_id: Hash256,
        peer_id: Hash256,
        local_balance: u128,
        remote_balance: u128,
        status: ChannelStatus,
    ) -> Self {
        Self {
            channel_id,
            peer_id,
            local_balance,
            remote_balance,
            status,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize ChannelState")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Key with prefix for storage
    pub fn key(&self) -> Vec<u8> {
        let mut key = vec![prefix::CHANNEL_STATE];
        key.extend_from_slice(&self.peer_id);
        key.extend_from_slice(&self.channel_id);
        key
    }
}

/// Payment session for tracking in-flight payments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentSession {
    pub payment_hash: Hash256,
    pub amount: u128,
    pub status: PaymentStatus,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending,
    Succeeded,
    Failed,
    Expired,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "Pending",
            PaymentStatus::Succeeded => "Succeeded",
            PaymentStatus::Failed => "Failed",
            PaymentStatus::Expired => "Expired",
        }
    }
}

impl PaymentSession {
    pub fn new(payment_hash: Hash256, amount: u128, status: PaymentStatus) -> Self {
        Self {
            payment_hash,
            amount,
            status,
            created_at: 0,
            expires_at: 0,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize PaymentSession")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    pub fn key(&self) -> Vec<u8> {
        let mut key = vec![prefix::PAYMENT_SESSION];
        key.extend_from_slice(&self.payment_hash);
        key
    }
}

/// Information about a connected peer
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    pub peer_id: Hash256,
    pub address: String,
    pub connected_at: u64,
    pub last_seen: u64,
}

impl PeerInfo {
    pub fn new(peer_id: Hash256, address: impl Into<String>) -> Self {
        Self {
            peer_id,
            address: address.into(),
            connected_at: 0,
            last_seen: 0,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize PeerInfo")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    pub fn key(&self) -> Vec<u8> {
        let mut key = vec![prefix::PEER_INFO];
        key.extend_from_slice(&self.peer_id);
        key
    }
}

/// Invoice for receiving payments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Invoice {
    pub payment_hash: Hash256,
    pub amount: u128,
    pub description: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub paid: bool,
}

impl Invoice {
    pub fn new(payment_hash: Hash256, amount: u128, description: impl Into<String>) -> Self {
        Self {
            payment_hash,
            amount,
            description: description.into(),
            created_at: 0,
            expires_at: 0,
            paid: false,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize Invoice")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    pub fn key(&self) -> Vec<u8> {
        let mut key = vec![prefix::INVOICE];
        key.extend_from_slice(&self.payment_hash);
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_state_roundtrip() {
        let state = ChannelState::new(
            [0x11; 32],
            [0x22; 32],
            1_000_000,
            2_000_000,
            ChannelStatus::Open,
        );
        let encoded = state.encode();
        let decoded = ChannelState::decode(&encoded).unwrap();
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_payment_session_roundtrip() {
        let session = PaymentSession::new([0xaa; 32], 50_000, PaymentStatus::Pending);
        let encoded = session.encode();
        let decoded = PaymentSession::decode(&encoded).unwrap();
        assert_eq!(session, decoded);
    }

    #[test]
    fn test_key_prefix() {
        let state = ChannelState::new([0x11; 32], [0x22; 32], 0, 0, ChannelStatus::Open);
        let key = state.key();
        assert_eq!(key[0], prefix::CHANNEL_STATE);

        let session = PaymentSession::new([0xaa; 32], 0, PaymentStatus::Pending);
        let key = session.key();
        assert_eq!(key[0], prefix::PAYMENT_SESSION);
    }
}
