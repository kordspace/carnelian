use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<f64>,
    operation: String,
    #[serde(default)]
    initial: Option<f64>,
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

    if input.array.is_empty() && input.initial.is_none() {
        let error = ErrorOutput {
            error: "Array is empty and no initial value provided".to_string(),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let result = match input.operation.to_lowercase().as_str() {
        "sum" | "add" => {
            let init = input.initial.unwrap_or(0.0);
            input.array.iter().fold(init, |acc, &x| acc + x)
        }
        "multiply" | "product" => {
            let init = input.initial.unwrap_or(1.0);
            input.array.iter().fold(init, |acc, &x| acc * x)
        }
        "min" => {
            let init = input.initial.unwrap_or(f64::INFINITY);
            input.array.iter().fold(init, |acc, &x| acc.min(x))
        }
        "max" => {
            let init = input.initial.unwrap_or(f64::NEG_INFINITY);
            input.array.iter().fold(init, |acc, &x| acc.max(x))
        }
        _ => {
            let error = ErrorOutput {
                error: format!("Unknown operation: {}", input.operation),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output { result };
    println!("{}", serde_json::to_string(&output).unwrap());
}
