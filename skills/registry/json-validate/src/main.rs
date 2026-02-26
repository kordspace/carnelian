use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: serde_json::Value,
    schema: serde_json::Value,
}

#[derive(Serialize)]
struct Output {
    valid: bool,
    errors: Vec<String>,
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

    let compiled_schema = match JSONSchema::compile(&input.schema) {
        Ok(schema) => schema,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid JSON schema: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let validation_result = compiled_schema.validate(&input.data);

    let (valid, errors) = match validation_result {
        Ok(_) => (true, Vec::new()),
        Err(validation_errors) => {
            let error_messages: Vec<String> = validation_errors
                .map(|e| format!("{}", e))
                .collect();
            (false, error_messages)
        }
    };

    let output = Output { valid, errors };
    println!("{}", serde_json::to_string(&output).unwrap());
}
