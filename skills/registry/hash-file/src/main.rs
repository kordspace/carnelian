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

    let content = match parsed.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: content"
            });
            println!("{}", error);
            return;
        }
    };

    let encoding = parsed.get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("utf8");

    let bytes = match encoding {
        "utf8" => content.as_bytes().to_vec(),
        "base64" => match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content) {
            Ok(b) => b,
            Err(e) => {
                let error = json!({
                    "success": false,
                    "error": format!("Base64 decode error: {}", e)
                });
                println!("{}", error);
                return;
            }
        },
        _ => {
            let error = json!({
                "success": false,
                "error": format!("Invalid encoding: {}. Must be 'utf8' or 'base64'", encoding)
            });
            println!("{}", error);
            return;
        }
    };

    let hash = blake3::hash(&bytes);
    let hash_hex = hash.to_hex();

    let result = json!({
        "success": true,
        "data": {
            "hash": hash_hex.to_string(),
            "algorithm": "blake3",
            "bytes": bytes.len()
        }
    });

    println!("{}", result);
}
