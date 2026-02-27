use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    encoded: String,
}

#[derive(Serialize)]
struct Output {
    data: String,
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

    let bytes = match hex::decode(&input.encoded) {
        Ok(b) => b,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to decode hex: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let data = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid UTF-8 data: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let length = data.len();

    let output = Output { data, length };
    println!("{}", serde_json::to_string(&output).unwrap());
}
