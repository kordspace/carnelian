use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: Value,
    keys: Vec<String>,
    #[serde(default)]
    exclude: bool,
}

#[derive(Serialize)]
struct Output {
    filtered: Value,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn filter_object(obj: &serde_json::Map<String, Value>, keys: &[String], exclude: bool) -> Value {
    let mut result = serde_json::Map::new();
    
    for (key, value) in obj {
        let should_include = if exclude {
            !keys.contains(key)
        } else {
            keys.contains(key)
        };
        
        if should_include {
            result.insert(key.clone(), value.clone());
        }
    }
    
    Value::Object(result)
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

    let filtered = match input.data {
        Value::Object(ref obj) => filter_object(obj, &input.keys, input.exclude),
        Value::Array(ref arr) => {
            let filtered_arr: Vec<Value> = arr
                .iter()
                .filter_map(|item| {
                    if let Value::Object(ref obj) = item {
                        Some(filter_object(obj, &input.keys, input.exclude))
                    } else {
                        Some(item.clone())
                    }
                })
                .collect();
            Value::Array(filtered_arr)
        }
        _ => input.data.clone(),
    };

    let output = Output { filtered };
    println!("{}", serde_json::to_string(&output).unwrap());
}
