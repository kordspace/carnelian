use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    start: Option<i32>,
    end: Option<i32>,
}

#[derive(Serialize)]
struct Output {
    result: Vec<Value>,
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

    let len = input.array.len() as i32;
    let start = input.start.unwrap_or(0);
    let end = input.end.unwrap_or(len);

    let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
    let end_idx = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;

    let result = if start_idx < end_idx {
        input.array[start_idx..end_idx].to_vec()
    } else {
        Vec::new()
    };

    let output = Output { result };
    println!("{}", serde_json::to_string(&output).unwrap());
}
