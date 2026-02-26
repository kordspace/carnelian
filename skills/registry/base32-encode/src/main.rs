use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
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

    let encoded = base32::encode(base32::Alphabet::RFC4648 { padding: true }, input.data.as_bytes());

    let output = Output { encoded };
    println!("{}", serde_json::to_string(&output).unwrap());
}
