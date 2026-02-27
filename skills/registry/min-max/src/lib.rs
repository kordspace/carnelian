use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub numbers: Vec<f64>,
}

#[derive(Serialize)]
pub struct Output {
    pub min: f64,
    pub max: f64,
    pub range: f64,
}

pub fn execute(input: Input) -> Result<Output, String> {
    if input.numbers.is_empty() {
        return Err("Empty input".to_string());
    }
    
    let min = input.numbers.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max = input.numbers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let range = max - min;
    
    Ok(Output { min, max, range })
}
