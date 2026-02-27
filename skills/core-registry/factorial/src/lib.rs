use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub n: u32,
}

#[derive(Serialize)]
pub struct Output {
    pub factorial: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    if input.n > 20 {
        return Err("n too large (max 20)".to_string());
    }
    
    let result = factorial(input.n);
    Ok(Output { factorial: result.to_string() })
}

fn factorial(n: u32) -> u64 {
    if n <= 1 {
        1
    } else {
        n as u64 * factorial(n - 1)
    }
}
