use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub metaphone: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let text = input.text.to_uppercase();
    let chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    
    if chars.is_empty() {
        return Ok(Output { metaphone: String::new() });
    }
    
    let mut result = String::new();
    let mut i = 0;
    
    // Skip initial silent letters
    while i < chars.len() && matches!(chars[i], 'K' | 'G' | 'P' | 'W') {
        if i + 1 < chars.len() && chars[i + 1] == 'N' {
            i += 1;
        }
        break;
    }
    
    while i < chars.len() && result.len() < 4 {
        let c = chars[i];
        let code = match c {
            'A' | 'E' | 'I' | 'O' | 'U' => if i == 0 { Some(c) } else { None },
            'B' => Some('B'),
            'C' => Some('K'),
            'D' => Some('T'),
            'F' => Some('F'),
            'G' => Some('K'),
            'H' => if i == 0 || is_vowel(chars.get(i.wrapping_sub(1))) { Some('H') } else { None },
            'J' => Some('J'),
            'K' => Some('K'),
            'L' => Some('L'),
            'M' => Some('M'),
            'N' => Some('N'),
            'P' => Some('P'),
            'Q' => Some('K'),
            'R' => Some('R'),
            'S' => Some('S'),
            'T' => Some('T'),
            'V' => Some('F'),
            'W' | 'Y' => if is_vowel(chars.get(i + 1)) { Some(c) } else { None },
            'X' => Some('K'),
            'Z' => Some('S'),
            _ => None,
        };
        
        if let Some(code) = code {
            if result.is_empty() || result.chars().last() != Some(code) {
                result.push(code);
            }
        }
        i += 1;
    }
    
    Ok(Output { metaphone: result })
}

fn is_vowel(c: Option<&char>) -> bool {
    matches!(c, Some('A') | Some('E') | Some('I') | Some('O') | Some('U'))
}
