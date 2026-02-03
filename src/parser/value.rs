use crate::config::ValueFormat;

pub struct ParseResult {
    pub content: String,
    pub success: bool,
}

pub fn parse_value(data: &[u8], format: &ValueFormat) -> ParseResult {
    match format {
        ValueFormat::String => parse_string(data),
        ValueFormat::Hex => ParseResult {
            content: super::hex_encode(data),
            success: true,
        },
        ValueFormat::Json => parse_json(data),
        ValueFormat::Msgpack => parse_msgpack(data),
        ValueFormat::Protobuf => ParseResult {
            content: format!("protobuf: {} bytes (handled separately)", data.len()),
            success: false,
        },
        ValueFormat::Molecule => ParseResult {
            content: format!("molecule: {} bytes (handled separately)", data.len()),
            success: false,
        },
    }
}

fn parse_string(data: &[u8]) -> ParseResult {
    match String::from_utf8(data.to_vec()) {
        Ok(s) => ParseResult {
            content: s,
            success: true,
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}

fn parse_json(data: &[u8]) -> ParseResult {
    match serde_json::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => ParseResult {
                content: s,
                success: true,
            },
            Err(_) => ParseResult {
                content: super::hex_encode(data),
                success: false,
            },
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}

fn parse_msgpack(data: &[u8]) -> ParseResult {
    match rmp_serde::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => ParseResult {
                content: s,
                success: true,
            },
            Err(_) => ParseResult {
                content: super::hex_encode(data),
                success: false,
            },
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}
