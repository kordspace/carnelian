use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub palindrome: bool,
    pub normalized: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    // Remove non-alphanumeric and convert to lowercase
    let normalized: String = input.text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    
    let palindrome = normalized == normalized.chars().rev().collect::<String>();
    
    Ok(Output {
        palindrome,
        normalized,
    })
}
