use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    encoded: String,
    #[serde(default)]
    url_safe: bool,
}

#[derive(Serialize)]
struct Output {
    data: String,
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

    let decoded_bytes = if input.url_safe {
        general_purpose::URL_SAFE.decode(input.encoded.as_bytes())
    } else {
        general_purpose::STANDARD.decode(input.encoded.as_bytes())
    };

    let decoded_bytes = match decoded_bytes {
        Ok(b) => b,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to decode base64: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let data = match String::from_utf8(decoded_bytes) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Decoded data is not valid UTF-8: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output { data };
    println!("{}", serde_json::to_string(&output).unwrap());
}
