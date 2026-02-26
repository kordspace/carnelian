use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    old_text: String,
    new_text: String,
    #[serde(default)]
    unified: bool,
    #[serde(default = "default_context")]
    context: usize,
}

fn default_context() -> usize {
    3
}

#[derive(Serialize)]
struct Change {
    tag: String,
    old_index: Option<usize>,
    new_index: Option<usize>,
    value: String,
}

#[derive(Serialize)]
struct Output {
    changes: Vec<Change>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unified_diff: Option<String>,
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

    let diff = TextDiff::from_lines(&input.old_text, &input.new_text);

    let mut changes = Vec::new();
    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Delete => "delete",
            ChangeTag::Insert => "insert",
            ChangeTag::Equal => "equal",
        };

        changes.push(Change {
            tag: tag.to_string(),
            old_index: change.old_index(),
            new_index: change.new_index(),
            value: change.value().to_string(),
        });
    }

    let unified_diff = if input.unified {
        Some(
            diff.unified_diff()
                .context_radius(input.context)
                .header("old", "new")
                .to_string(),
        )
    } else {
        None
    };

    let output = Output {
        changes,
        unified_diff,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
