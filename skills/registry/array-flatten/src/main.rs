use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Value,
    #[serde(default)]
    depth: Option<usize>,
}

#[derive(Serialize)]
struct Output {
    flattened: Vec<Value>,
    original_length: usize,
    flattened_length: usize,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn flatten_array(arr: &Value, depth: usize, current_depth: usize) -> Vec<Value> {
    let mut result = Vec::new();
    
    if let Value::Array(items) = arr {
        for item in items {
            if current_depth < depth && item.is_array() {
                result.extend(flatten_array(item, depth, current_depth + 1));
            } else {
                result.push(item.clone());
            }
        }
    } else {
        result.push(arr.clone());
    }
    
    result
}

fn count_items(arr: &Value) -> usize {
    if let Value::Array(items) = arr {
        items.len()
    } else {
        1
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

    let depth = input.depth.unwrap_or(usize::MAX);
    let original_length = count_items(&input.array);
    let flattened = flatten_array(&input.array, depth, 0);
    let flattened_length = flattened.len();

    let output = Output {
        flattened,
        original_length,
        flattened_length,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
