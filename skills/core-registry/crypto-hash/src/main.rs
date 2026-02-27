use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest, Sha256, Sha512};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
}

fn default_algorithm() -> String {
    "sha256".to_string()
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

    let hash = match input.algorithm.to_lowercase().as_str() {
        "md5" => {
            let digest = md5::compute(input.data.as_bytes());
            format!("{:x}", digest)
        }
        "sha1" => {
            let mut hasher = Sha1::new();
            hasher.update(input.data.as_bytes());
            hex::encode(hasher.finalize())
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(input.data.as_bytes());
            hex::encode(hasher.finalize())
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(input.data.as_bytes());
            hex::encode(hasher.finalize())
        }
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported algorithm: {}", input.algorithm),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        hash,
        algorithm: input.algorithm,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
