use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default)]
    url_safe: bool,
}

#[derive(Serialize)]
struct Output {
    encoded: String,
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

    let encoded = if input.url_safe {
        general_purpose::URL_SAFE.encode(input.data.as_bytes())
    } else {
        general_purpose::STANDARD.encode(input.data.as_bytes())
    };

    let output = Output { encoded };
    println!("{}", serde_json::to_string(&output).unwrap());
}
