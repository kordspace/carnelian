use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub numbers: Vec<f64>,
}

#[derive(Serialize)]
pub struct Output {
    pub mean: f64,
    pub median: f64,
    pub mode: Vec<f64>,
    pub std_dev: f64,
    pub variance: f64,
    pub min: f64,
    pub max: f64,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let numbers = input.numbers;
    
    if numbers.is_empty() {
        return Err("Empty input array".to_string());
    }
    
    let count = numbers.len();
    let sum: f64 = numbers.iter().sum();
    let mean = sum / count as f64;
    
    let mut sorted = numbers.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let median = if count % 2 == 0 {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
    } else {
        sorted[count / 2]
    };
    
    // Mode calculation
    let mut frequencies = std::collections::HashMap::new();
    for &num in &numbers {
        *frequencies.entry(num.to_string()).or_insert(0) += 1;
    }
    
    let max_freq = frequencies.values().max().copied().unwrap_or(0);
    let mode: Vec<f64> = frequencies.iter()
        .filter(|(_, &freq)| freq == max_freq)
        .map(|(key, _)| key.parse::<f64>().unwrap())
        .collect();
    
    let variance: f64 = numbers.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / count as f64;
    
    let std_dev = variance.sqrt();
    
    Ok(Output {
        mean,
        median,
        mode,
        std_dev,
        variance,
        min: sorted[0],
        max: sorted[count - 1],
        count,
    })
}
