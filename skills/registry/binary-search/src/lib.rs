use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub numbers: Vec<i64>,
    pub target: i64,
}

#[derive(Serialize)]
pub struct Output {
    pub found: bool,
    pub index: Option<usize>,
    pub iterations: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let arr = input.numbers;
    let target = input.target;
    
    let mut left = 0;
    let mut right = arr.len();
    let mut iterations = 0;
    
    while left < right {
        iterations += 1;
        let mid = left + (right - left) / 2;
        
        if arr[mid] == target {
            return Ok(Output { found: true, index: Some(mid), iterations });
        } else if arr[mid] < target {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    
    Ok(Output { found: false, index: None, iterations })
}
