use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Cursor, Read};

#[derive(Deserialize)]
struct Input {
    data: Value,
    root_element: String,
    #[serde(default = "default_true")]
    declaration: bool,
    #[serde(default)]
    indent: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Output {
    xml_data: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn write_value<W: std::io::Write>(
    writer: &mut Writer<W>,
    tag: &str,
    value: &Value,
) -> Result<(), String> {
    match value {
        Value::String(s) => {
            let elem = BytesStart::new(tag);
            writer.write_event(Event::Start(elem)).map_err(|e| e.to_string())?;
            writer
                .write_event(Event::Text(BytesText::new(s)))
                .map_err(|e| e.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new(tag)))
                .map_err(|e| e.to_string())?;
        }
        Value::Number(n) => {
            let elem = BytesStart::new(tag);
            writer.write_event(Event::Start(elem)).map_err(|e| e.to_string())?;
            writer
                .write_event(Event::Text(BytesText::new(&n.to_string())))
                .map_err(|e| e.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new(tag)))
                .map_err(|e| e.to_string())?;
        }
        Value::Bool(b) => {
            let elem = BytesStart::new(tag);
            writer.write_event(Event::Start(elem)).map_err(|e| e.to_string())?;
            writer
                .write_event(Event::Text(BytesText::new(&b.to_string())))
                .map_err(|e| e.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new(tag)))
                .map_err(|e| e.to_string())?;
        }
        Value::Object(map) => {
            let elem = BytesStart::new(tag);
            writer.write_event(Event::Start(elem)).map_err(|e| e.to_string())?;
            for (key, val) in map {
                write_value(writer, key, val)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new(tag)))
                .map_err(|e| e.to_string())?;
        }
        Value::Array(arr) => {
            for item in arr {
                write_value(writer, tag, item)?;
            }
        }
        Value::Null => {
            let elem = BytesStart::new(tag);
            writer.write_event(Event::Empty(elem)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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

    let mut buffer = Vec::new();
    let mut writer = if let Some(indent_size) = input.indent {
        Writer::new_with_indent(Cursor::new(&mut buffer), b' ', indent_size)
    } else {
        Writer::new(Cursor::new(&mut buffer))
    };

    if input.declaration {
        if let Err(e) = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        {
            let error = ErrorOutput {
                error: format!("Failed to write XML declaration: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }

    if let Err(e) = write_value(&mut writer, &input.root_element, &input.data) {
        let error = ErrorOutput { error: e };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let xml_data = match String::from_utf8(buffer) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to convert XML to UTF-8: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output { xml_data };
    println!("{}", serde_json::to_string(&output).unwrap());
}
