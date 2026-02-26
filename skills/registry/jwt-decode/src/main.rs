use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    token: String,
    secret: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    verify: bool,
}

#[derive(Serialize)]
struct Output {
    payload: serde_json::Value,
    header: serde_json::Value,
    valid: bool,
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

    let algorithm = match input.algorithm.as_str() {
        "HS256" | "" => Algorithm::HS256,
        "HS384" => Algorithm::HS384,
        "HS512" => Algorithm::HS512,
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported algorithm: {}", input.algorithm),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let mut validation = Validation::new(algorithm);
    if !input.verify {
        validation.insecure_disable_signature_validation();
    }

    let token_data = match decode::<serde_json::Value>(
        &input.token,
        &DecodingKey::from_secret(input.secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => data,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to decode JWT: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        payload: token_data.claims,
        header: serde_json::to_value(&token_data.header).unwrap(),
        valid: true,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
