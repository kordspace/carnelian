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
    sum: f64,
    count: usize,
    average: f64,
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

    let mut sum = 0.0;
    let mut count = 0;

    for item in &input.array {
        if let Some(num) = extract_number(item, &input.key) {
            sum += num;
            count += 1;
        }
    }

    let average = if count > 0 { sum / count as f64 } else { 0.0 };

    let output = Output {
        sum,
        count,
        average,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
