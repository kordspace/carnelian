use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
    pub key: String,
}

#[derive(Serialize)]
pub struct Output {
    pub encrypted: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let text_bytes = input.text.as_bytes();
    let key_bytes = input.key.as_bytes();
    
    if key_bytes.is_empty() {
        return Err("Key cannot be empty".to_string());
    }
    
    let encrypted: Vec<u8> = text_bytes.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    
    let encrypted = base64::encode(&encrypted);
    
    Ok(Output { encrypted })
}

mod base64 {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    pub fn encode(data: &[u8]) -> String {
        let mut result = String::new();
        let mut i = 0;
        
        while i < data.len() {
            let b1 = data[i];
            let b2 = if i + 1 < data.len() { data[i + 1] } else { 0 };
            let b3 = if i + 2 < data.len() { data[i + 2] } else { 0 };
            
            let idx1 = (b1 >> 2) as usize;
            let idx2 = (((b1 & 0x3) << 4) | (b2 >> 4)) as usize;
            let idx3 = (((b2 & 0xF) << 2) | (b3 >> 6)) as usize;
            let idx4 = (b3 & 0x3F) as usize;
            
            result.push(ALPHABET[idx1] as char);
            result.push(ALPHABET[idx2] as char);
            
            if i + 1 < data.len() {
                result.push(ALPHABET[idx3] as char);
            } else {
                result.push('=');
            }
            
            if i + 2 < data.len() {
                result.push(ALPHABET[idx4] as char);
            } else {
                result.push('=');
            }
            
            i += 3;
        }
        
        result
    }
}
