use serde_json::{json, Value};
use std::io::{self, Read};
use pulldown_cmark::{Event, Parser, Tag, TagEnd, HeadingLevel};

fn main() {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        let error = json!({
            "success": false,
            "error": format!("Failed to read stdin: {}", e)
        });
        println!("{}", error);
        return;
    }

    let parsed: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let error = json!({
                "success": false,
                "error": format!("Invalid JSON input: {}", e)
            });
            println!("{}", error);
            return;
        }
    };

    let content = match parsed.get("content").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: content"
            });
            println!("{}", error);
            return;
        }
    };

    let parser = Parser::new(content);
    let mut ast_events = Vec::new();
    let mut headings = Vec::new();
    
    let mut in_heading = false;
    let mut heading_level: u32 = 0;
    let mut heading_text = String::new();
    
    for event in parser {
        match &event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                heading_text.clear();
                ast_events.push(json!({
                    "type": "heading_start",
                    "level": heading_level
                }));
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_heading {
                    headings.push(json!({
                        "level": heading_level,
                        "text": heading_text.clone()
                    }));
                    in_heading = false;
                }
                ast_events.push(json!({
                    "type": "heading_end"
                }));
            }
            Event::Start(Tag::Paragraph) => {
                ast_events.push(json!({
                    "type": "paragraph_start"
                }));
            }
            Event::End(TagEnd::Paragraph) => {
                ast_events.push(json!({
                    "type": "paragraph_end"
                }));
            }
            Event::Text(text) => {
                if in_heading {
                    heading_text.push_str(text);
                }
                ast_events.push(json!({
                    "type": "text",
                    "content": text.to_string()
                }));
            }
            Event::Code(code) => {
                if in_heading {
                    heading_text.push_str(code);
                }
                ast_events.push(json!({
                    "type": "code",
                    "content": code.to_string()
                }));
            }
            Event::SoftBreak => {
                if in_heading {
                    heading_text.push(' ');
                }
                ast_events.push(json!({
                    "type": "soft_break"
                }));
            }
            Event::HardBreak => {
                if in_heading {
                    heading_text.push('\n');
                }
                ast_events.push(json!({
                    "type": "hard_break"
                }));
            }
            Event::Html(html) => {
                if in_heading {
                    heading_text.push_str(html);
                }
                ast_events.push(json!({
                    "type": "html",
                    "content": html.to_string()
                }));
            }
            _ => {
                ast_events.push(json!({
                    "type": "other"
                }));
            }
        }
    }

    let result = json!({
        "success": true,
        "data": {
            "ast": ast_events,
            "headings": headings
        }
    });

    println!("{}", result);
}
