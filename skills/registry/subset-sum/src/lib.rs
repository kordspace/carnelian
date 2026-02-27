use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub data: Vec<f64>,
    pub target: f64,
}

#[derive(Serialize)]
pub struct Output {
    pub combinations: Vec<Vec<f64>>,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut combinations = Vec::new();
    let data = input.data;
    let target = input.target;
    
    find_combinations(&data, target, &mut Vec::new(), &mut combinations);
    
    let count = combinations.len();
    Ok(Output { combinations, count })
}

fn find_combinations(
    data: &[f64],
    target: f64,
    current: &mut Vec<f64>,
    results: &mut Vec<Vec<f64>>,
) {
    if current.len() > 10 {
        return;
    }
    
    let sum: f64 = current.iter().sum();
    if (sum - target).abs() < 0.0001 && !current.is_empty() {
        results.push(current.clone());
        return;
    }
    
    for (i, &val) in data.iter().enumerate() {
        if current.len() < 10 {
            current.push(val);
            find_combinations(&data[i..], target, current, results);
            current.pop();
        }
    }
}
