mod key;
pub mod key_schema;
pub mod molecule;
pub mod protobuf;
mod value;

pub use key::{parse_key_hex, parse_key_with_schema};
pub use key_schema::KeySchemaRegistry;
pub use molecule::MoleculeRegistry;
pub use protobuf::ProtoRegistry;
pub use value::parse_value;

pub(crate) fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
