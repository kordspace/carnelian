use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    min: Option<i64>,
    #[serde(default)]
    max: Option<i64>,
    #[serde(default)]
    count: Option<usize>,
}

#[derive(Serialize)]
struct Output {
    numbers: Vec<i64>,
    count: usize,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn generate_random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).unwrap();
    u64::from_le_bytes(bytes)
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

    let min = input.min.unwrap_or(0);
    let max = input.max.unwrap_or(100);
    let count = input.count.unwrap_or(1);

    if min >= max {
        let error = ErrorOutput {
            error: "min must be less than max".to_string(),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let range = (max - min) as u64;
    let mut numbers = Vec::new();

    for _ in 0..count {
        let random = generate_random_u64();
        let num = min + (random % range) as i64;
        numbers.push(num);
    }

    let output = Output {
        count: numbers.len(),
        numbers,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
