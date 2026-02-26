use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    array: Vec<Value>,
    #[serde(default)]
    descending: bool,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Serialize)]
struct Output {
    sorted: Vec<Value>,
    length: usize,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn compare_values(a: &Value, b: &Value, descending: bool) -> std::cmp::Ordering {
    let order = match (a, b) {
        (Value::Number(n1), Value::Number(n2)) => {
            let f1 = n1.as_f64().unwrap_or(0.0);
            let f2 = n2.as_f64().unwrap_or(0.0);
            f1.partial_cmp(&f2).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(s1), Value::String(s2)) => s1.cmp(s2),
        (Value::Bool(b1), Value::Bool(b2)) => b1.cmp(b2),
        _ => std::cmp::Ordering::Equal,
    };
    
    if descending {
        order.reverse()
    } else {
        order
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

    let mut sorted = input.array.clone();

    if let Some(key) = input.key {
        sorted.sort_by(|a, b| {
            let val_a = a.get(&key).unwrap_or(&Value::Null);
            let val_b = b.get(&key).unwrap_or(&Value::Null);
            compare_values(val_a, val_b, input.descending)
        });
    } else {
        sorted.sort_by(|a, b| compare_values(a, b, input.descending));
    }

    let length = sorted.len();
    let output = Output { sorted, length };

    println!("{}", serde_json::to_string(&output).unwrap());
}
