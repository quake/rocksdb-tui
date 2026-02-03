mod key;
mod value;

pub use key::parse_key;
pub use value::parse_value;
pub use value::ParseResult;

pub(crate) fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
