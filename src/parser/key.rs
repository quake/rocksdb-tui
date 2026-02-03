use crate::parser::key_schema::KeySchema;

/// Parse a key using a schema, or fall back to hex
pub fn parse_key_with_schema(data: &[u8], schema: Option<&KeySchema>) -> String {
    match schema {
        Some(s) => s.decode(data),
        None => super::hex_encode(data),
    }
}

/// Simple hex encoding for fallback
pub fn parse_key_hex(data: &[u8]) -> String {
    super::hex_encode(data)
}
