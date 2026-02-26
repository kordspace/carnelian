use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    number: f64,
    #[serde(default)]
    decimals: Option<usize>,
    #[serde(default)]
    thousands_separator: Option<String>,
    #[serde(default)]
    decimal_separator: Option<String>,
}

#[derive(Serialize)]
struct Output {
    formatted: String,
    original: f64,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn format_number(
    num: f64,
    decimals: usize,
    thousands_sep: &str,
    decimal_sep: &str,
) -> String {
    let rounded = format!("{:.prec$}", num, prec = decimals);
    let parts: Vec<&str> = rounded.split('.').collect();
    
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "" };
    
    // Add thousands separators
    let mut formatted_integer = String::new();
    let chars: Vec<char> = integer_part.chars().collect();
    let len = chars.len();
    
    for (i, ch) in chars.iter().enumerate() {
        formatted_integer.push(*ch);
        let pos = len - i - 1;
        if pos > 0 && pos % 3 == 0 && !thousands_sep.is_empty() {
            formatted_integer.push_str(thousands_sep);
        }
    }
    
    if decimals > 0 && !decimal_part.is_empty() {
        format!("{}{}{}", formatted_integer, decimal_sep, decimal_part)
    } else {
        formatted_integer
    }
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

    let decimals = input.decimals.unwrap_or(2);
    let thousands_sep = input.thousands_separator.as_deref().unwrap_or(",");
    let decimal_sep = input.decimal_separator.as_deref().unwrap_or(".");

    let formatted = format_number(input.number, decimals, thousands_sep, decimal_sep);

    let output = Output {
        formatted,
        original: input.number,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
