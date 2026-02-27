use md5::{Digest as Md5Digest, Md5};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default)]
    algorithm: String,
}

#[derive(Serialize)]
struct Output {
    checksum: String,
    algorithm: String,
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

    let algorithm = if input.algorithm.is_empty() {
        "sha256".to_string()
    } else {
        input.algorithm.to_lowercase()
    };

    let data_bytes = input.data.as_bytes();
    let size = data_bytes.len();

    let checksum = match algorithm.as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(data_bytes);
            hex::encode(hasher.finalize())
        }
        "md5" => {
            let mut hasher = Md5::new();
            hasher.update(data_bytes);
            hex::encode(hasher.finalize())
        }
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported algorithm: {}", algorithm),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        checksum,
        algorithm,
        size,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
