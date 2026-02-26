use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    color: String,
    #[serde(default = "default_format")]
    to_format: String,
}

fn default_format() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Output {
    hex: Option<String>,
    rgb: Option<RGB>,
    hsl: Option<HSL>,
}

#[derive(Serialize)]
struct RGB {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Serialize)]
struct HSL {
    h: f32,
    s: f32,
    l: f32,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8), String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Hex color must be 6 characters".to_string());
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;

    Ok((r, g, b))
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) / 2.0;

    if delta == 0.0 {
        return (0.0, 0.0, l);
    }

    let s = if l < 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    let h = if max == r {
        ((g - b) / delta + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    (h * 360.0, s * 100.0, l * 100.0)
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

    let (r, g, b) = match parse_hex(&input.color) {
        Ok(rgb) => rgb,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to parse color: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let (h, s, l) = rgb_to_hsl(r, g, b);

    let output = Output {
        hex: Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
        rgb: Some(RGB { r, g, b }),
        hsl: Some(HSL { h, s, l }),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
