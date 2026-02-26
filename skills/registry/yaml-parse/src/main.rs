use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        let error = json!({
            "success": false,
            "error": format!("Failed to read stdin: {}", e)
        });
        println!("{}", error);
        return;
    }

    let parsed: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let error = json!({
                "success": false,
                "error": format!("Invalid JSON input: {}", e)
            });
            println!("{}", error);
            return;
        }
    };

    if let Some(yaml_str) = parsed.get("content").and_then(|v| v.as_str()) {
        match serde_yaml::from_str::<Value>(yaml_str) {
            Ok(json_value) => {
                let result = json!({
                    "success": true,
                    "data": {
                        "json": json_value
                    }
                });
                println!("{}", result);
            }
            Err(e) => {
                let error = json!({
                    "success": false,
                    "error": format!("YAML parse error: {}", e)
                });
                println!("{}", error);
            }
        }
    } else if let Some(json_value) = parsed.get("json") {
        let to_yaml = parsed.get("to_yaml").and_then(|v| v.as_bool()).unwrap_or(false);
        
        if to_yaml {
            match serde_yaml::to_string(json_value) {
                Ok(yaml_str) => {
                    let result = json!({
                        "success": true,
                        "data": {
                            "yaml": yaml_str
                        }
                    });
                    println!("{}", result);
                }
                Err(e) => {
                    let error = json!({
                        "success": false,
                        "error": format!("YAML serialization error: {}", e)
                    });
                    println!("{}", error);
                }
            }
        } else {
            let error = json!({
                "success": false,
                "error": "to_yaml must be true when json field is provided"
            });
            println!("{}", error);
        }
    } else {
        let error = json!({
            "success": false,
            "error": "Missing required field: content or json"
        });
        println!("{}", error);
    }
}
