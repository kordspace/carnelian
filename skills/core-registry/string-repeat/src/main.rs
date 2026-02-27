use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text: String,
    count: usize,
    #[serde(default)]
    separator: String,
}

#[derive(Serialize)]
struct Output {
    result: String,
    length: usize,
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

    if input.count > 10000 {
        let error = ErrorOutput {
            error: "Count exceeds maximum limit of 10000".to_string(),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let result = if input.separator.is_empty() {
        input.text.repeat(input.count)
    } else {
        (0..input.count)
            .map(|_| input.text.as_str())
            .collect::<Vec<_>>()
            .join(&input.separator)
    };

    let length = result.len();

    let output = Output { result, length };
    println!("{}", serde_json::to_string(&output).unwrap());
}
