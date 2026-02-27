use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    base: Value,
    overlay: Value,
    #[serde(default)]
    deep: bool,
}

#[derive(Serialize)]
struct Output {
    merged: Value,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn merge_json(base: &mut Value, overlay: &Value, deep: bool) {
    if let (Some(base_obj), Some(overlay_obj)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            if deep && base_obj.contains_key(key) {
                if let Some(base_value) = base_obj.get_mut(key) {
                    merge_json(base_value, value, deep);
                    continue;
                }
            }
            base_obj.insert(key.clone(), value.clone());
        }
    } else {
        *base = overlay.clone();
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

    let mut merged = input.base.clone();
    merge_json(&mut merged, &input.overlay, input.deep);

    let output = Output { merged };
    println!("{}", serde_json::to_string(&output).unwrap());
}
