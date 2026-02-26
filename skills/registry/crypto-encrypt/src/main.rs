use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    key: String,
    #[serde(default)]
    algorithm: String,
}

#[derive(Serialize)]
struct Output {
    encrypted: String,
    nonce: String,
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

    // Decode the key from base64
    let key_bytes = match general_purpose::STANDARD.decode(&input.key) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 key: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    if key_bytes.len() != 32 {
        let error = ErrorOutput {
            error: format!("Key must be 32 bytes (256 bits), got {}", key_bytes.len()),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let cipher = match Aes256Gcm::new_from_slice(&key_bytes) {
        Ok(c) => c,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to create cipher: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    // Generate random nonce
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // Encrypt the data
    let ciphertext = match cipher.encrypt(&nonce, input.data.as_bytes()) {
        Ok(ct) => ct,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Encryption failed: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        encrypted: general_purpose::STANDARD.encode(&ciphertext),
        nonce: general_purpose::STANDARD.encode(&nonce),
        algorithm: "AES-256-GCM".to_string(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
