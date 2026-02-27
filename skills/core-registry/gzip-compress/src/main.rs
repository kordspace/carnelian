use base64::{engine::general_purpose, Engine as _};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default)]
    level: Option<u32>,
}

#[derive(Serialize)]
struct Output {
    compressed: String,
    original_size: usize,
    compressed_size: usize,
    ratio: f64,
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

    let compression_level = match input.level {
        Some(l) if l <= 9 => Compression::new(l),
        Some(_) => Compression::default(),
        None => Compression::default(),
    };

    let data_bytes = input.data.as_bytes();
    let original_size = data_bytes.len();

    let mut encoder = GzEncoder::new(Vec::new(), compression_level);
    if let Err(e) = encoder.write_all(data_bytes) {
        let error = ErrorOutput {
            error: format!("Failed to compress data: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let compressed_bytes = match encoder.finish() {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to finish compression: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let compressed_size = compressed_bytes.len();
    let ratio = if original_size > 0 {
        (compressed_size as f64 / original_size as f64) * 100.0
    } else {
        0.0
    };

    let compressed_base64 = general_purpose::STANDARD.encode(&compressed_bytes);

    let output = Output {
        compressed: compressed_base64,
        original_size,
        compressed_size,
        ratio,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
