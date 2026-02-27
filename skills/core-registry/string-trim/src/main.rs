use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text: String,
    #[serde(default)]
    chars: Option<String>,
    #[serde(default)]
    side: String,
}

#[derive(Serialize)]
struct Output {
    trimmed: String,
    removed_chars: usize,
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

    let original_len = input.text.len();
    
    let trimmed = if let Some(chars) = input.chars {
        let chars_to_trim: Vec<char> = chars.chars().collect();
        match input.side.to_lowercase().as_str() {
            "left" | "start" => input.text.trim_start_matches(&chars_to_trim[..]).to_string(),
            "right" | "end" => input.text.trim_end_matches(&chars_to_trim[..]).to_string(),
            _ => input.text.trim_matches(&chars_to_trim[..]).to_string(),
        }
    } else {
        match input.side.to_lowercase().as_str() {
            "left" | "start" => input.text.trim_start().to_string(),
            "right" | "end" => input.text.trim_end().to_string(),
            _ => input.text.trim().to_string(),
        }
    };

    let removed_chars = original_len - trimmed.len();

    let output = Output {
        trimmed,
        removed_chars,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
