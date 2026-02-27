use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
    pub chars: String,
}

#[derive(Serialize)]
pub struct Output {
    pub result: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let chars: Vec<char> = input.chars.chars().collect();
    let result: String = input.text.chars()
        .filter(|c| !chars.contains(c))
        .collect();
    
    Ok(Output { result })
}
