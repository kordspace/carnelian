use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    private_key: String,
}

#[derive(Serialize)]
struct Output {
    signature: String,
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

    // Decode the private key from base64
    let key_bytes = match general_purpose::STANDARD.decode(&input.private_key) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 private key: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    if key_bytes.len() != 32 {
        let error = ErrorOutput {
            error: format!("Private key must be 32 bytes, got {}", key_bytes.len()),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&key_bytes);

    let signing_key = SigningKey::from_bytes(&key_array);
    let signature: Signature = signing_key.sign(input.data.as_bytes());

    let output = Output {
        signature: general_purpose::STANDARD.encode(signature.to_bytes()),
        algorithm: "Ed25519".to_string(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
