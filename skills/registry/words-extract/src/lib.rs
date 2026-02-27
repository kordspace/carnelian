use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub words: Vec<String>,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let words: Vec<String> = input.text
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let count = words.len();
    Ok(Output { words, count })
}
