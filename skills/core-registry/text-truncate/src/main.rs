use serde::{Deserialize, Serialize};
use std::io::Read;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Deserialize)]
struct Input {
    text: String,
    max_length: usize,
    #[serde(default)]
    ellipsis: String,
    #[serde(default)]
    word_boundary: bool,
}

#[derive(Serialize)]
struct Output {
    truncated: String,
    was_truncated: bool,
    original_length: usize,
    final_length: usize,
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

    let ellipsis = if input.ellipsis.is_empty() {
        "...".to_string()
    } else {
        input.ellipsis
    };

    let graphemes: Vec<&str> = input.text.graphemes(true).collect();
    let original_length = graphemes.len();

    if original_length <= input.max_length {
        let output = Output {
            truncated: input.text.clone(),
            was_truncated: false,
            original_length,
            final_length: original_length,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
        return;
    }

    let ellipsis_len = ellipsis.graphemes(true).count();
    let available_length = if input.max_length > ellipsis_len {
        input.max_length - ellipsis_len
    } else {
        input.max_length
    };

    let mut truncated = if input.word_boundary {
        let partial: String = graphemes.iter().take(available_length).copied().collect();
        if let Some(last_space) = partial.rfind(char::is_whitespace) {
            partial[..last_space].trim_end().to_string()
        } else {
            partial
        }
    } else {
        graphemes.iter().take(available_length).copied().collect()
    };

    if available_length < input.max_length {
        truncated.push_str(&ellipsis);
    }

    let final_length = truncated.graphemes(true).count();

    let output = Output {
        truncated,
        was_truncated: true,
        original_length,
        final_length,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
