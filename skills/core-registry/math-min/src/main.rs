use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    values: Vec<f64>,
}

#[derive(Serialize)]
struct Output {
    result: f64,
    index: usize,
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

    if input.values.is_empty() {
        let error = ErrorOutput {
            error: "values array cannot be empty".to_string(),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let mut min_value = input.values[0];
    let mut min_index = 0;

    for (i, &value) in input.values.iter().enumerate().skip(1) {
        if value < min_value {
            min_value = value;
            min_index = i;
        }
    }

    let output = Output {
        result: min_value,
        index: min_index,
    };
    println!("{}", serde_json::to_string(&output).unwrap());
}
