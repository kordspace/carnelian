use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    timestamp: i64,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Serialize)]
struct Output {
    formatted: String,
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

    let dt = match DateTime::from_timestamp(input.timestamp, 0) {
        Some(dt) => dt,
        None => {
            let error = ErrorOutput {
                error: format!("Invalid timestamp: {}", input.timestamp),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let formatted = if let Some(fmt) = input.format {
        dt.format(&fmt).to_string()
    } else {
        dt.to_rfc3339()
    };

    let output = Output { formatted };
    println!("{}", serde_json::to_string(&output).unwrap());
}
