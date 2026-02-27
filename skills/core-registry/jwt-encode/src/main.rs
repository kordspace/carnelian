use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    payload: serde_json::Value,
    secret: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Serialize)]
struct Output {
    token: String,
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

    let mut claims = input.payload;
    
    // Add expiration if specified
    if let Some(exp_seconds) = input.expires_in {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        if let Some(obj) = claims.as_object_mut() {
            obj.insert("exp".to_string(), serde_json::json!(now + exp_seconds));
        }
    }

    let token = match encode(
        &Header::new(algorithm),
        &claims,
        &EncodingKey::from_secret(input.secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to encode JWT: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        token,
        algorithm: format!("{:?}", algorithm),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
