use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    template: String,
    data: serde_json::Value,
}

#[derive(Serialize)]
struct Output {
    rendered: String,
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

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(false);

    let rendered = match handlebars.render_template(&input.template, &input.data) {
        Ok(r) => r,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to render template: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output { rendered };
    println!("{}", serde_json::to_string(&output).unwrap());
}
