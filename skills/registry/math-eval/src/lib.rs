use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub expression: String,
}

#[derive(Serialize)]
pub struct Output {
    pub result: f64,
    pub error: Option<String>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    // Simple arithmetic expression evaluator
    let result = evaluate_expression(&input.expression);
    
    match result {
        Ok(value) => Ok(Output { result: value, error: None }),
        Err(e) => Ok(Output { result: 0.0, error: Some(e) }),
    }
}

fn evaluate_expression(expr: &str) -> Result<f64, String> {
    // Remove whitespace
    let expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    
    if expr.is_empty() {
        return Err("Empty expression".to_string());
    }
    
    // Simple number parsing
    match expr.parse::<f64>() {
        Ok(n) => return Ok(n),
        Err(_) => {}
    }
    
    // Find operator
    for (i, c) in expr.chars().enumerate() {
        if matches!(c, '+' | '-' | '*' | '/' | '^') && i > 0 {
            let left = &expr[..i];
            let right = &expr[i+1..];
            
            let left_val = evaluate_expression(left)?;
            let right_val = evaluate_expression(right)?;
            
            let result = match c {
                '+' => left_val + right_val,
                '-' => left_val - right_val,
                '*' => left_val * right_val,
                '/' => {
                    if right_val == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    left_val / right_val
                }
                '^' => left_val.powf(right_val),
                _ => return Err("Unknown operator".to_string()),
            };
            
            return Ok(result);
        }
    }
    
    Err("Invalid expression".to_string())
}
