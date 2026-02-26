use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    html: String,
    selector: String,
    #[serde(default)]
    extract: String,
}

#[derive(Serialize)]
struct Element {
    text: String,
    html: String,
    attributes: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct Output {
    elements: Vec<Element>,
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

    let document = Html::parse_document(&input.html);

    let selector = match Selector::parse(&input.selector) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid CSS selector: {:?}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let mut elements = Vec::new();

    for element in document.select(&selector) {
        let text = element.text().collect::<Vec<_>>().join("");
        let html = element.html();
        
        let mut attributes = std::collections::HashMap::new();
        for (name, value) in element.value().attrs() {
            attributes.insert(name.to_string(), value.to_string());
        }

        elements.push(Element {
            text,
            html,
            attributes,
        });
    }

    let count = elements.len();
    let output = Output { elements, count };

    println!("{}", serde_json::to_string(&output).unwrap());
}
