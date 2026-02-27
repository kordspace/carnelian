use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Serialize)]
struct Output {
    count: usize,
    sum: f64,
    mean: f64,
    min: f64,
    max: f64,
    median: f64,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn extract_number(value: &Value, key: &Option<String>) -> Option<f64> {
    if let Some(k) = key {
        value.get(k)?.as_f64()
    } else {
        value.as_f64()
    }
}

fn main() {
    let mut input_str = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input_str) {
        let error = ErrorOutput {
            error: format!("Failed to read input: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let input: Input = match serde_json::from_str(&input_str) {
        Ok(i) => i,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid JSON input: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let mut numbers: Vec<f64> = input
        .array
        .iter()
        .filter_map(|item| extract_number(item, &input.key))
        .collect();

    if numbers.is_empty() {
        let output = Output {
            count: 0,
            sum: 0.0,
            mean: 0.0,
            min: 0.0,
            max: 0.0,
            median: 0.0,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
        return;
    }

    let count = numbers.len();
    let sum: f64 = numbers.iter().sum();
    let mean = sum / count as f64;
    let min = numbers.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = numbers.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if count % 2 == 0 {
        (numbers[count / 2 - 1] + numbers[count / 2]) / 2.0
    } else {
        numbers[count / 2]
    };

    let output = Output {
        count,
        sum,
        mean,
        min,
        max,
        median,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
