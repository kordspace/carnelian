use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text: String,
    length: usize,
    #[serde(default)]
    pad_char: String,
    #[serde(default)]
    direction: String,
}

#[derive(Serialize)]
struct Output {
    padded: String,
    original_length: usize,
    final_length: usize,
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

    let pad_char = if input.pad_char.is_empty() {
        " "
    } else {
        &input.pad_char
    };

    let direction = if input.direction.is_empty() {
        "right"
    } else {
        &input.direction.to_lowercase()
    };

    let original_length = input.text.len();
    
    let padded = if original_length >= input.length {
        input.text.clone()
    } else {
        let pad_count = input.length - original_length;
        let padding: String = pad_char.repeat(pad_count);
        
        match direction {
            "left" => format!("{}{}", padding, input.text),
            "right" => format!("{}{}", input.text, padding),
            "both" => {
                let left_pad = pad_count / 2;
                let right_pad = pad_count - left_pad;
                format!(
                    "{}{}{}",
                    pad_char.repeat(left_pad),
                    input.text,
                    pad_char.repeat(right_pad)
                )
            }
            _ => input.text.clone(),
        }
    };

    let final_length = padded.len();

    let output = Output {
        padded,
        original_length,
        final_length,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
