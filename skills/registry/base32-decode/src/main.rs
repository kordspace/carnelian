use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    encoded: String,
}

#[derive(Serialize)]
struct Output {
    decoded: String,
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

    match base32::decode(base32::Alphabet::RFC4648 { padding: true }, &input.encoded) {
        Some(bytes) => {
            match String::from_utf8(bytes) {
                Ok(decoded) => {
                    let output = Output { decoded };
                    println!("{}", serde_json::to_string(&output).unwrap());
                }
                Err(e) => {
                    let error = ErrorOutput {
                        error: format!("Invalid UTF-8 in decoded data: {}", e),
                    };
                    println!("{}", serde_json::to_string(&error).unwrap());
                    std::process::exit(1);
                }
            }
        }
        None => {
            let error = ErrorOutput {
                error: "Invalid base32 input".to_string(),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }
}
