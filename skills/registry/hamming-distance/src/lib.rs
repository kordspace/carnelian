use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub string1: String,
    pub string2: String,
}

#[derive(Serialize)]
pub struct Output {
    pub distance: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    if input.string1.len() != input.string2.len() {
        return Err("Strings must be of equal length for Hamming distance".to_string());
    }
    
    let distance = input.string1.chars()
        .zip(input.string2.chars())
        .filter(|(a, b)| a != b)
        .count();
    
    Ok(Output { distance })
}
