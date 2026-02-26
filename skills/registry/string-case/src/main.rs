use heck::{
    ToKebabCase, ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase,
    ToUpperCamelCase,
};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text: String,
    case: String,
}

#[derive(Serialize)]
struct Output {
    result: String,
    case: String,
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

    let result = match input.case.to_lowercase().as_str() {
        "snake" | "snake_case" => input.text.to_snake_case(),
        "kebab" | "kebab-case" => input.text.to_kebab_case(),
        "camel" | "camelcase" => input.text.to_lower_camel_case(),
        "pascal" | "pascalcase" => input.text.to_pascal_case(),
        "upper" | "uppercase" => input.text.to_uppercase(),
        "lower" | "lowercase" => input.text.to_lowercase(),
        "title" | "titlecase" => input.text.to_title_case(),
        "screaming_snake" | "shouty_snake" => input.text.to_shouty_snake_case(),
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported case type: {}", input.case),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        result,
        case: input.case,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
