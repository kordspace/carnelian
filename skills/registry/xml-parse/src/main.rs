use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    xml_data: String,
    #[serde(default)]
    trim_text: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
enum XmlValue {
    Text(String),
    Object(std::collections::HashMap<String, XmlValue>),
    Array(Vec<XmlValue>),
}

#[derive(Serialize)]
struct Output {
    data: XmlValue,
    root_element: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn parse_xml(xml_data: &str, trim_text: bool) -> Result<(String, XmlValue), String> {
    let mut reader = Reader::from_str(xml_data);
    reader.config_mut().trim_text(trim_text);
    
    let mut stack: Vec<(String, std::collections::HashMap<String, XmlValue>)> = Vec::new();
    let mut root_name = String::new();
    let mut current_text = String::new();
    
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if root_name.is_empty() {
                    root_name = name.clone();
                }
                stack.push((name, std::collections::HashMap::new()));
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    current_text.push_str(&text);
                }
            }
            Ok(Event::End(_)) => {
                if let Some((tag_name, mut element)) = stack.pop() {
                    let value = if !current_text.is_empty() {
                        XmlValue::Text(current_text.clone())
                    } else if !element.is_empty() {
                        XmlValue::Object(element.clone())
                    } else {
                        XmlValue::Text(String::new())
                    };
                    
                    current_text.clear();
                    
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.entry(tag_name.clone())
                            .and_modify(|existing| {
                                match existing {
                                    XmlValue::Array(arr) => arr.push(value.clone()),
                                    _ => {
                                        let old = std::mem::replace(existing, XmlValue::Array(vec![]));
                                        if let XmlValue::Array(arr) = existing {
                                            arr.push(old);
                                            arr.push(value.clone());
                                        }
                                    }
                                }
                            })
                            .or_insert(value);
                    } else {
                        return Ok((root_name, XmlValue::Object(element)));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }
    
    Ok((root_name, XmlValue::Object(std::collections::HashMap::new())))
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

    match parse_xml(&input.xml_data, input.trim_text) {
        Ok((root, data)) => {
            let output = Output {
                data,
                root_element: root,
            };
            println!("{}", serde_json::to_string(&output).unwrap());
        }
        Err(e) => {
            let error = ErrorOutput { error: e };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }
}
