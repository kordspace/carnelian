use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Input {
    pub morse: String,
}

#[derive(Serialize)]
pub struct Output {
    pub text: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let morse_map: HashMap<&str, char> = [
        (".-", 'A'), ("-...", 'B'), ("-.-.", 'C'), ("-..", 'D'), (".", 'E'),
        ("..-.", 'F'), ("--.", 'G'), ("....", 'H'), ("..", 'I'), (".---", 'J'),
        ("-.-", 'K'), (".-..", 'L'), ("--", 'M'), ("-.", 'N'), ("---", 'O'),
        (".--.", 'P'), ("--.-", 'Q'), (".-.", 'R'), ("...", 'S'), ("-", 'T'),
        ("..-", 'U'), ("...-", 'V'), (".--", 'W'), ("-..-", 'X'), ("-.--", 'Y'),
        ("--..", 'Z'), ("-----", '0'), (".----", '1'), ("..---", '2'),
        ("...--", '3'), ("....-", '4'), (".....", '5'), ("-....", '6'),
        ("--...", '7'), ("---..", '8'), ("----.", '9'), ("/", ' '),
    ].iter().cloned().collect();
    
    let text: String = input.morse.split_whitespace()
        .filter_map(|code| morse_map.get(code).copied())
        .collect();
    
    Ok(Output { text })
}
