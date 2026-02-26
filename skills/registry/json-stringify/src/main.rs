use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: Value,
    #[serde(default)]
    pretty: bool,
    #[serde(default)]
    indent: Option<usize>,
}

#[derive(Serialize)]
struct Output {
    json: String,
    size: usize,
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

    let json = if input.pretty {
        let indent_str = " ".repeat(input.indent.unwrap_or(2));
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        if input.data.serialize(&mut ser).is_err() {
            let error = ErrorOutput {
                error: "Failed to serialize data".to_string(),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
        String::from_utf8(buf).unwrap()
    } else {
        match serde_json::to_string(&input.data) {
            Ok(s) => s,
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Failed to stringify JSON: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    };

    let size = json.len();

    let output = Output { json, size };
    println!("{}", serde_json::to_string(&output).unwrap());
}
