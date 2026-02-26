use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    signature: String,
    public_key: String,
}

#[derive(Serialize)]
struct Output {
    valid: bool,
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

    // Decode the public key from base64
    let key_bytes = match general_purpose::STANDARD.decode(&input.public_key) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 public key: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    if key_bytes.len() != 32 {
        let error = ErrorOutput {
            error: format!("Public key must be 32 bytes, got {}", key_bytes.len()),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&key_bytes);

    let verifying_key = match VerifyingKey::from_bytes(&key_array) {
        Ok(key) => key,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid public key: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    // Decode the signature from base64
    let sig_bytes = match general_purpose::STANDARD.decode(&input.signature) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 signature: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    if sig_bytes.len() != 64 {
        let error = ErrorOutput {
            error: format!("Signature must be 64 bytes, got {}", sig_bytes.len()),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let signature = match Signature::from_slice(&sig_bytes) {
        Ok(sig) => sig,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid signature: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let valid = verifying_key.verify(input.data.as_bytes(), &signature).is_ok();

    let output = Output {
        valid,
        algorithm: "Ed25519".to_string(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
