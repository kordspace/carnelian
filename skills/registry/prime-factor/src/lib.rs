use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub n: u64,
}

#[derive(Serialize)]
pub struct Output {
    pub is_prime: bool,
    pub factors: Vec<u64>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let n = input.n;
    
    if n < 2 {
        return Ok(Output { is_prime: false, factors: vec![] });
    }
    
    let mut factors = Vec::new();
    let mut num = n;
    let mut divisor = 2;
    
    while divisor * divisor <= num {
        while num % divisor == 0 {
            factors.push(divisor);
            num /= divisor;
        }
        divisor += 1;
    }
    
    if num > 1 {
        factors.push(num);
    }
    
    let is_prime = factors.len() == 1 && factors[0] == n;
    
    Ok(Output { is_prime, factors })
}
