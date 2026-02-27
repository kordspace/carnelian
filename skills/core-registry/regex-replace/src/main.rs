use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    pattern: String,
    replacement: String,
    text: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Serialize)]
struct Output {
    result: String,
    replacements: usize,
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

    let mut pattern = input.pattern.clone();
    if input.case_insensitive {
        pattern = format!("(?i){}", pattern);
    }
    if input.multiline {
        pattern = format!("(?m){}", pattern);
    }

    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid regex pattern: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let (result, replacements) = if input.replace_all {
        let count = re.find_iter(&input.text).count();
        let result = re.replace_all(&input.text, input.replacement.as_str()).to_string();
        (result, count)
    } else {
        let count = if re.is_match(&input.text) { 1 } else { 0 };
        let result = re.replace(&input.text, input.replacement.as_str()).to_string();
        (result, count)
    };

    let output = Output {
        result,
        replacements,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
