use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    bytes: Vec<u8>,
    #[serde(default)]
    encoding: String,
}

#[derive(Serialize)]
struct Output {
    text: String,
    byte_count: usize,
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

    let encoding = if input.encoding.is_empty() {
        "utf8"
    } else {
        &input.encoding.to_lowercase()
    };

    let text = match encoding {
        "utf8" | "utf-8" => match String::from_utf8(input.bytes.clone()) {
            Ok(s) => s,
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Invalid UTF-8: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        },
        "ascii" => {
            input
                .bytes
                .iter()
                .map(|&b| if b < 128 { b as char } else { '?' })
                .collect()
        }
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported encoding: {}", encoding),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let byte_count = input.bytes.len();

    let output = Output { text, byte_count };
    println!("{}", serde_json::to_string(&output).unwrap());
}
