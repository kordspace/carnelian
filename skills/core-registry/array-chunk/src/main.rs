use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    size: usize,
}

#[derive(Serialize)]
struct Output {
    chunks: Vec<Vec<Value>>,
    chunk_count: usize,
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

    if input.size == 0 {
        let error = ErrorOutput {
            error: "Chunk size must be greater than 0".to_string(),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let chunks: Vec<Vec<Value>> = input
        .array
        .chunks(input.size)
        .map(|chunk| chunk.to_vec())
        .collect();

    let chunk_count = chunks.len();

    let output = Output {
        chunks,
        chunk_count,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
