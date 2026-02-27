use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub result: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let result: String = input.text.chars()
        .map(|c| {
            match c {
                'A'..='M' | 'a'..='m' => ((c as u8) + 13) as char,
                'N'..='Z' | 'n'..='z' => ((c as u8) - 13) as char,
                _ => c,
            }
        })
        .collect();
    
    Ok(Output { result })
}
