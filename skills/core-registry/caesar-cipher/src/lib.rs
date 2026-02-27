use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
    pub shift: i32,
}

#[derive(Serialize)]
pub struct Output {
    pub result: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let shift = input.shift.rem_euclid(26);
    
    let result: String = input.text.chars()
        .map(|c| {
            match c {
                'A'..='Z' => {
                    let offset = (c as u8 - b'A') as i32;
                    let new_offset = (offset + shift).rem_euclid(26);
                    (b'A' + new_offset as u8) as char
                }
                'a'..='z' => {
                    let offset = (c as u8 - b'a') as i32;
                    let new_offset = (offset + shift).rem_euclid(26);
                    (b'a' + new_offset as u8) as char
                }
                _ => c,
            }
        })
        .collect();
    
    Ok(Output { result })
}
