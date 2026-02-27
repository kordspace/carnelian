use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub encoded: String,
}

#[derive(Serialize)]
pub struct Output {
    pub decoded: String,
}

const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub fn execute(input: Input) -> Result<Output, String> {
    let mut num = 0u128;
    
    for c in input.encoded.bytes() {
        let pos = ALPHABET.iter().position(|&b| b == c)
            .ok_or_else(|| format!("Invalid Base58 character: {}", c as char))?;
        num = num * 58 + pos as u128;
    }
    
    let mut bytes = Vec::new();
    while num > 0 {
        bytes.push((num % 256) as u8);
        num /= 256;
    }
    
    // Add leading zero bytes for leading '1's
    for c in input.encoded.bytes() {
        if c == b'1' {
            bytes.push(0);
        } else {
            break;
        }
    }
    
    bytes.reverse();
    
    Ok(Output {
        decoded: String::from_utf8(bytes).map_err(|e| e.to_string())?,
    })
}
