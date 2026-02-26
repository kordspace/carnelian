use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    left: Value,
    right: Value,
}

#[derive(Serialize)]
struct Output {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
    unchanged: Vec<String>,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn compare_values(left: &Value, right: &Value, path: String, output: &mut Output) {
    match (left, right) {
        (Value::Object(left_obj), Value::Object(right_obj)) => {
            // Check for removed and changed keys
            for (key, left_val) in left_obj {
                let new_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                if let Some(right_val) = right_obj.get(key) {
                    if left_val != right_val {
                        if left_val.is_object() && right_val.is_object() {
                            compare_values(left_val, right_val, new_path, output);
                        } else {
                            output.changed.push(new_path);
                        }
                    } else {
                        output.unchanged.push(new_path);
                    }
                } else {
                    output.removed.push(new_path);
                }
            }

            // Check for added keys
            for key in right_obj.keys() {
                if !left_obj.contains_key(key) {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    output.added.push(new_path);
                }
            }
        }
        _ => {
            if left != right {
                output.changed.push(path);
            } else {
                output.unchanged.push(path);
            }
        }
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

    let mut output = Output {
        added: Vec::new(),
        removed: Vec::new(),
        changed: Vec::new(),
        unchanged: Vec::new(),
    };

    compare_values(&input.left, &input.right, String::new(), &mut output);

    println!("{}", serde_json::to_string(&output).unwrap());
}
