use base64::{engine::general_purpose, Engine as _};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    compressed: String,
}

#[derive(Serialize)]
struct Output {
    data: String,
    compressed_size: usize,
    decompressed_size: usize,
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

    let compressed_bytes = match general_purpose::STANDARD.decode(&input.compressed) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to decode base64: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let compressed_size = compressed_bytes.len();

    let mut decoder = GzDecoder::new(&compressed_bytes[..]);
    let mut decompressed_data = String::new();

    if let Err(e) = decoder.read_to_string(&mut decompressed_data) {
        let error = ErrorOutput {
            error: format!("Failed to decompress data: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let decompressed_size = decompressed_data.len();

    let output = Output {
        data: decompressed_data,
        compressed_size,
        decompressed_size,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
