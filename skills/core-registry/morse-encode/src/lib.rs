use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub morse: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let morse_map: HashMap<char, &str> = [
        ('A', ".-"), ('B', "-..."), ('C', "-.-."), ('D', "-.."), ('E', "."),
        ('F', "..-."), ('G', "--."), ('H', "...."), ('I', ".."), ('J', ".---"),
        ('K', "-.-"), ('L', ".-.."), ('M', "--"), ('N', "-."), ('O', "---"),
        ('P', ".--."), ('Q', "--.-"), ('R', ".-."), ('S', "..."), ('T', "-"),
        ('U', "..-"), ('V', "...-"), ('W', ".--"), ('X', "-..-"), ('Y', "-.--"),
        ('Z', "--.."), ('0', "-----"), ('1', ".----"), ('2', "..---"),
        ('3', "...--"), ('4', "....-"), ('5', "....."), ('6', "-...."),
        ('7', "--..."), ('8', "---.."), ('9', "----."), (' ', "/"),
    ].iter().cloned().collect();
    
    let morse: Vec<String> = input.text.to_uppercase().chars()
        .filter_map(|c| morse_map.get(&c).map(|&s| s.to_string()))
        .collect();
    
    Ok(Output {
        morse: morse.join(" "),
    })
}
