use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
    pub width: usize,
}

#[derive(Serialize)]
pub struct Output {
    pub wrapped: String,
    pub lines: Vec<String>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let text = input.text;
    let width = input.width;
    
    let mut lines = Vec::new();
    let mut current_line = String::new();
    
    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    let wrapped = lines.join("\n");
    
    Ok(Output { wrapped, lines })
}
