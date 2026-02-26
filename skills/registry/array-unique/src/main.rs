use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Serialize)]
struct Output {
    unique: Vec<Value>,
    original_length: usize,
    unique_length: usize,
    duplicates_removed: usize,
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

    let original_length = input.array.len();
    let mut unique = Vec::new();
    let mut seen = HashSet::new();

    for item in input.array {
        let key_value = if let Some(ref key) = input.key {
            item.get(key).unwrap_or(&Value::Null).to_string()
        } else {
            item.to_string()
        };

        if seen.insert(key_value) {
            unique.push(item);
        }
    }

    let unique_length = unique.len();
    let duplicates_removed = original_length - unique_length;

    let output = Output {
        unique,
        original_length,
        unique_length,
        duplicates_removed,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
