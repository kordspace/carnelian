use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub soundex: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let text = input.text.to_uppercase();
    let chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    
    if chars.is_empty() {
        return Ok(Output { soundex: String::new() });
    }
    
    let mut result = String::new();
    result.push(chars[0]);
    
    let mut prev_code = char_to_soundex(chars[0]);
    
    for &c in &chars[1..] {
        let code = char_to_soundex(c);
        if code != '0' && code != prev_code {
            result.push(code);
            if result.len() == 4 {
                break;
            }
        }
        prev_code = code;
    }
    
    while result.len() < 4 {
        result.push('0');
    }
    
    Ok(Output { soundex: result })
}

fn char_to_soundex(c: char) -> char {
    match c {
        'B' | 'F' | 'P' | 'V' => '1',
        'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
        'D' | 'T' => '3',
        'L' => '4',
        'M' | 'N' => '5',
        'R' => '6',
        _ => '0',
    }
}
