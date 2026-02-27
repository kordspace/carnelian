use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub list: Vec<String>,
    pub separator: String,
}

#[derive(Serialize)]
pub struct Output {
    pub result: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let result = input.list.join(&input.separator);
    Ok(Output { result })
}
