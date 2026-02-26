use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    key: String,
    value: Value,
}

#[derive(Serialize)]
struct Output {
    found: Option<Value>,
    index: Option<usize>,
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

    let mut found = None;
    let mut index = None;

    for (i, item) in input.array.iter().enumerate() {
        if let Some(obj) = item.as_object() {
            if let Some(item_value) = obj.get(&input.key) {
                if item_value == &input.value {
                    found = Some(item.clone());
                    index = Some(i);
                    break;
                }
            }
        }
    }

    let output = Output { found, index };
    println!("{}", serde_json::to_string(&output).unwrap());
}
