use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    duration: String,
}

#[derive(Serialize)]
struct Output {
    seconds: u64,
    minutes: u64,
    hours: u64,
    days: u64,
    total_seconds: u64,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn parse_duration(s: &str) -> Result<u64, String> {
    let s = s.trim().to_lowercase();
    let mut total_seconds = 0u64;
    let mut current_num = String::new();
    
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else if ch.is_alphabetic() {
            if current_num.is_empty() {
                return Err("Invalid duration format".to_string());
            }
            
            let num: u64 = current_num.parse().map_err(|_| "Invalid number")?;
            current_num.clear();
            
            let multiplier = match ch {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                _ => return Err(format!("Unknown unit: {}", ch)),
            };
            
            total_seconds += num * multiplier;
        }
    }
    
    if !current_num.is_empty() {
        let num: u64 = current_num.parse().map_err(|_| "Invalid number")?;
        total_seconds += num;
    }
    
    Ok(total_seconds)
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

    let total_seconds = match parse_duration(&input.duration) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput { error: e };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        total_seconds,
        seconds: total_seconds % 60,
        minutes: (total_seconds / 60) % 60,
        hours: (total_seconds / 3600) % 24,
        days: total_seconds / 86400,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
