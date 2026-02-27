use serde::{Deserialize, Serialize};
use std::io::Read;
use uuid::Uuid;

#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    count: Option<usize>,
    #[serde(default)]
    uppercase: bool,
}

#[derive(Serialize)]
struct Output {
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    uuids: Option<Vec<String>>,
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

    let count = input.count.unwrap_or(1);

    if count == 1 {
        let uuid = Uuid::new_v4();
        let uuid_str = if input.uppercase {
            uuid.to_string().to_uppercase()
        } else {
            uuid.to_string()
        };

        let output = Output {
            uuid: uuid_str,
            uuids: None,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        let uuids: Vec<String> = (0..count)
            .map(|_| {
                let uuid = Uuid::new_v4();
                if input.uppercase {
                    uuid.to_string().to_uppercase()
                } else {
                    uuid.to_string()
                }
            })
            .collect();

        let output = Output {
            uuid: uuids[0].clone(),
            uuids: Some(uuids),
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    }
}
