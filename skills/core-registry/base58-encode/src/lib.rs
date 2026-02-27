use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub data: String,
}

#[derive(Serialize)]
pub struct Output {
    pub encoded: String,
}

const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub fn execute(input: Input) -> Result<Output, String> {
    let bytes = input.data.as_bytes();
    let mut result = Vec::new();
    
    let mut num = bytes.iter().fold(0u128, |acc, &b| acc * 256 + b as u128);
    
    while num > 0 {
        result.push(ALPHABET[(num % 58) as usize]);
        num /= 58;
    }
    
    // Add leading '1's for leading zero bytes
    for &b in bytes {
        if b == 0 {
            result.push(b'1');
        } else {
            break;
        }
    }
    
    result.reverse();
    
    Ok(Output {
        encoded: String::from_utf8(result).map_err(|e| e.to_string())?,
    })
}
