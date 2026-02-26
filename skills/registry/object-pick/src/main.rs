use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    object: Value,
    keys: Vec<String>,
}

#[derive(Serialize)]
struct Output {
    picked: Value,
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

    let mut picked = serde_json::Map::new();

    if let Value::Object(obj) = input.object {
        for key in input.keys {
            if let Some(value) = obj.get(&key) {
                picked.insert(key, value.clone());
            }
        }
    }

    let output = Output {
        picked: Value::Object(picked),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
