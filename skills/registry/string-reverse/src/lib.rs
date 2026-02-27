use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub reversed: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let reversed: String = input.text.chars().rev().collect();
    Ok(Output { reversed })
}
