use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    key: String,
}

#[derive(Serialize)]
struct Output {
    groups: HashMap<String, Vec<Value>>,
    group_count: usize,
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

    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();

    for item in input.array {
        if let Some(obj) = item.as_object() {
            if let Some(group_value) = obj.get(&input.key) {
                let group_key = match group_value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    _ => serde_json::to_string(group_value).unwrap_or_else(|_| "unknown".to_string()),
                };

                groups.entry(group_key).or_insert_with(Vec::new).push(item);
            }
        }
    }

    let group_count = groups.len();

    let output = Output { groups, group_count };
    println!("{}", serde_json::to_string(&output).unwrap());
}
