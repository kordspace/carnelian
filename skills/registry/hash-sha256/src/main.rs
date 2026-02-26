use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default)]
    encoding: String,
}

#[derive(Serialize)]
struct Output {
    hash: String,
    algorithm: String,
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

    let bytes = if input.encoding.to_lowercase() == "hex" {
        match hex::decode(&input.data) {
            Ok(b) => b,
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Invalid hex encoding: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    } else {
        input.data.as_bytes().to_vec()
    };

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();
    let hash = hex::encode(result);

    let output = Output {
        hash,
        algorithm: "SHA-256".to_string(),
    };
    println!("{}", serde_json::to_string(&output).unwrap());
}
