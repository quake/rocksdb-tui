use crate::config::KeyFormat;

pub fn parse_key(data: &[u8], format: &KeyFormat) -> String {
    match format {
        KeyFormat::String => String::from_utf8_lossy(data).to_string(),
        KeyFormat::Hex => super::hex_encode(data),
        KeyFormat::U64Be => {
            if data.len() == 8 {
                let arr: [u8; 8] = data.try_into().unwrap();
                u64::from_be_bytes(arr).to_string()
            } else {
                super::hex_encode(data)
            }
        }
        KeyFormat::U64Le => {
            if data.len() == 8 {
                let arr: [u8; 8] = data.try_into().unwrap();
                u64::from_le_bytes(arr).to_string()
            } else {
                super::hex_encode(data)
            }
        }
    }
}
