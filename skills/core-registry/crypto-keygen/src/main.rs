use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    #[serde(default = "default_size")]
    size: usize,
    #[serde(default)]
    count: usize,
}

fn default_size() -> usize {
    32 // 256 bits
}

#[derive(Serialize)]
struct Output {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    keys: Option<Vec<String>>,
    size: usize,
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

    if input.size == 0 || input.size > 1024 {
        let error = ErrorOutput {
            error: format!("Key size must be between 1 and 1024 bytes, got {}", input.size),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let count = if input.count == 0 { 1 } else { input.count };

    if count == 1 {
        let mut key_bytes = vec![0u8; input.size];
        rand::thread_rng().fill_bytes(&mut key_bytes);
        let key = general_purpose::STANDARD.encode(&key_bytes);

        let output = Output {
            key,
            keys: None,
            size: input.size,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        let keys: Vec<String> = (0..count)
            .map(|_| {
                let mut key_bytes = vec![0u8; input.size];
                rand::thread_rng().fill_bytes(&mut key_bytes);
                general_purpose::STANDARD.encode(&key_bytes)
            })
            .collect();

        let output = Output {
            key: keys[0].clone(),
            keys: Some(keys),
            size: input.size,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    }
}
