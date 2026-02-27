use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    email: String,
}

#[derive(Serialize)]
struct Output {
    valid: bool,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
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

    // RFC 5322 compliant email regex (simplified)
    let email_regex = Regex::new(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
    ).unwrap();

    let valid = email_regex.is_match(&input.email);

    let (username, domain) = if valid {
        if let Some((user, dom)) = input.email.split_once('@') {
            (Some(user.to_string()), Some(dom.to_string()))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let output = Output {
        valid,
        email: input.email,
        username,
        domain,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
