use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub n: u32,
}

#[derive(Serialize)]
pub struct Output {
    pub fibonacci: Vec<u64>,
    pub nth: u64,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let n = input.n as usize;
    
    if n == 0 {
        return Ok(Output { fibonacci: vec![], nth: 0 });
    }
    
    let mut fib = Vec::with_capacity(n);
    fib.push(0u64);
    
    if n > 1 {
        fib.push(1u64);
    }
    
    for i in 2..n {
        let next = fib[i - 1] + fib[i - 2];
        fib.push(next);
    }
    
    let nth = *fib.last().unwrap_or(&0);
    
    Ok(Output { fibonacci: fib, nth })
}
