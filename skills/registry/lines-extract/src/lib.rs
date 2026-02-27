use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub lines: Vec<String>,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let lines: Vec<String> = input.text.lines().map(|s| s.to_string()).collect();
    let count = lines.len();
    Ok(Output { lines, count })
}
