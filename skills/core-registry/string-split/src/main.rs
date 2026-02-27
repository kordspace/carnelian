use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text: String,
    delimiter: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    regex: bool,
}

#[derive(Serialize)]
struct Output {
    parts: Vec<String>,
    count: usize,
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

    let parts: Vec<String> = if input.regex {
        let re = match Regex::new(&input.delimiter) {
            Ok(r) => r,
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Invalid regex pattern: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        };

        if let Some(limit) = input.limit {
            re.splitn(&input.text, limit)
                .map(|s| s.to_string())
                .collect()
        } else {
            re.split(&input.text).map(|s| s.to_string()).collect()
        }
    } else {
        if let Some(limit) = input.limit {
            input
                .text
                .splitn(limit, &input.delimiter)
                .map(|s| s.to_string())
                .collect()
        } else {
            input
                .text
                .split(&input.delimiter)
                .map(|s| s.to_string())
                .collect()
        }
    };

    let count = parts.len();
    let output = Output { parts, count };

    println!("{}", serde_json::to_string(&output).unwrap());
}
