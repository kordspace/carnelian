use base64::{engine::general_purpose, Engine as _};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: String,
    #[serde(default)]
    size: Option<usize>,
}

#[derive(Serialize)]
struct Output {
    svg: String,
    matrix: Vec<Vec<bool>>,
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

    let code = match QrCode::new(input.data.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to generate QR code: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(input.size.unwrap_or(200), input.size.unwrap_or(200))
        .build();

    let matrix: Vec<Vec<bool>> = code
        .to_colors()
        .chunks(code.width())
        .map(|row| row.iter().map(|&c| c == qrcode::Color::Dark).collect())
        .collect();

    let output = Output {
        svg,
        matrix,
        size: code.width(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
