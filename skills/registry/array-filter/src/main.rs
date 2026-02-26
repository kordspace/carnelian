use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    key: String,
    value: Value,
    #[serde(default)]
    operator: String,
}

#[derive(Serialize)]
struct Output {
    filtered: Vec<Value>,
    original_count: usize,
    filtered_count: usize,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn matches(item_value: &Value, target_value: &Value, operator: &str) -> bool {
    match operator {
        "eq" | "=" | "==" => item_value == target_value,
        "ne" | "!=" => item_value != target_value,
        "gt" | ">" => {
            if let (Some(a), Some(b)) = (item_value.as_f64(), target_value.as_f64()) {
                a > b
            } else {
                false
            }
        }
        "gte" | ">=" => {
            if let (Some(a), Some(b)) = (item_value.as_f64(), target_value.as_f64()) {
                a >= b
            } else {
                false
            }
        }
        "lt" | "<" => {
            if let (Some(a), Some(b)) = (item_value.as_f64(), target_value.as_f64()) {
                a < b
            } else {
                false
            }
        }
        "lte" | "<=" => {
            if let (Some(a), Some(b)) = (item_value.as_f64(), target_value.as_f64()) {
                a <= b
            } else {
                false
            }
        }
        "contains" => {
            if let (Some(a), Some(b)) = (item_value.as_str(), target_value.as_str()) {
                a.contains(b)
            } else {
                false
            }
        }
        _ => item_value == target_value,
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

    let operator = if input.operator.is_empty() {
        "eq"
    } else {
        &input.operator
    };

    let original_count = input.array.len();
    
    let filtered: Vec<Value> = input
        .array
        .into_iter()
        .filter(|item| {
            if let Some(obj) = item.as_object() {
                if let Some(item_value) = obj.get(&input.key) {
                    return matches(item_value, &input.value, operator);
                }
            }
            false
        })
        .collect();

    let filtered_count = filtered.len();

    let output = Output {
        filtered,
        original_count,
        filtered_count,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
