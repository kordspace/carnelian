use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    pattern: String,
    text: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    find_all: bool,
}

#[derive(Serialize)]
struct Match {
    text: String,
    start: usize,
    end: usize,
    groups: Vec<Option<String>>,
}

#[derive(Serialize)]
struct Output {
    matches: Vec<Match>,
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

    let mut matches = Vec::new();

    if input.find_all {
        for cap in re.captures_iter(&input.text) {
            let m = cap.get(0).unwrap();
            let groups: Vec<Option<String>> = cap
                .iter()
                .skip(1)
                .map(|g| g.map(|m| m.as_str().to_string()))
                .collect();

            matches.push(Match {
                text: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
                groups,
            });
        }
    } else if let Some(cap) = re.captures(&input.text) {
        let m = cap.get(0).unwrap();
        let groups: Vec<Option<String>> = cap
            .iter()
            .skip(1)
            .map(|g| g.map(|m| m.as_str().to_string()))
            .collect();

        matches.push(Match {
            text: m.as_str().to_string(),
            start: m.start(),
            end: m.end(),
            groups,
        });
    }

    let output = Output {
        count: matches.len(),
        matches,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
