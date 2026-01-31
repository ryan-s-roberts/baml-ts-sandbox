//! Test tool implementations for testing BAML tool system.

use baml_rt::tools::BamlTool;
use serde_json::{json, Value};
use baml_rt::Result;
use async_trait::async_trait;

/// Example calculator tool
pub struct CalculatorTool;

#[async_trait]
impl BamlTool for CalculatorTool {
    const NAME: &'static str = "calculate";
    
    fn description(&self) -> &'static str {
        "Performs mathematical calculations. Can handle addition, subtraction, multiplication, and division."
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "object",
                    "properties": {
                        "left": {"type": "integer"},
                        "operation": {
                            "type": "string",
                            "enum": ["Add", "Subtract", "Multiply", "Divide"]
                        },
                        "right": {"type": "integer"}
                    },
                    "required": ["left", "operation", "right"]
                }
            },
            "required": ["expression"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let obj = args.as_object().expect("Expected object");
        let expr_obj = obj.get("expression")
            .and_then(|v| v.as_object())
            .expect("Expected 'expression' object");
        
        let left = expr_obj.get("left")
            .and_then(|v| v.as_i64())
            .expect("Expected 'left' integer") as f64;
        let operation_enum = expr_obj.get("operation")
            .and_then(|v| v.as_str())
            .expect("Expected 'operation' string");
        let right = expr_obj.get("right")
            .and_then(|v| v.as_i64())
            .expect("Expected 'right' integer") as f64;
        
        // Map enum values to symbols
        let (operation_symbol, result) = match operation_enum {
            "Add" => ("+", left + right),
            "Subtract" => ("-", left - right),
            "Multiply" => ("*", left * right),
            "Divide" => ("/", if right != 0.0 { left / right } else { 0.0 }),
            _ => ("?", 0.0),
        };
        
        let expr_str = format!("{} {} {}", left as i64, operation_symbol, right as i64);
        tracing::info!(expression = %expr_str, "CalculatorTool executed");
        
        Ok(json!({
            "expression": expr_str,
            "result": result,
            "formatted": format!("{} = {}", expr_str, result)
        }))
    }
}
