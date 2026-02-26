use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    value: f64,
    #[serde(default)]
    precision: Option<i32>,
    #[serde(default)]
    mode: String,
}

#[derive(Serialize)]
struct Output {
    result: f64,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
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

    let precision = input.precision.unwrap_or(0);
    let multiplier = 10_f64.powi(precision);

    let result = match input.mode.to_lowercase().as_str() {
        "floor" => (input.value * multiplier).floor() / multiplier,
        "ceil" | "ceiling" => (input.value * multiplier).ceil() / multiplier,
        "trunc" | "truncate" => (input.value * multiplier).trunc() / multiplier,
        _ => (input.value * multiplier).round() / multiplier,
    };

    let output = Output { result };
    println!("{}", serde_json::to_string(&output).unwrap());
}
