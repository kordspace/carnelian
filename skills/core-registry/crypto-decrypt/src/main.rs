use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    encrypted: String,
    key: String,
    nonce: String,
}

#[derive(Serialize)]
struct Output {
    decrypted: String,
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

    // Decode the nonce from base64
    let nonce_bytes = match general_purpose::STANDARD.decode(&input.nonce) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 nonce: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    if nonce_bytes.len() != 12 {
        let error = ErrorOutput {
            error: format!("Nonce must be 12 bytes, got {}", nonce_bytes.len()),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let nonce = Nonce::from_slice(&nonce_bytes);

    // Decode the ciphertext from base64
    let ciphertext = match general_purpose::STANDARD.decode(&input.encrypted) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid base64 ciphertext: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

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

    // Decrypt the data
    let plaintext = match cipher.decrypt(nonce, ciphertext.as_ref()) {
        Ok(pt) => pt,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Decryption failed: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let decrypted = match String::from_utf8(plaintext) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Decrypted data is not valid UTF-8: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output { decrypted };
    println!("{}", serde_json::to_string(&output).unwrap());
}
