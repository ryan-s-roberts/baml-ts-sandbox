//! Example tools implementing the BamlTool trait

use baml_rt::tools::BamlTool;
use serde_json::{json, Value};
use baml_rt::error::Result;
use async_trait::async_trait;

/// Example weather tool
pub struct WeatherTool;

#[async_trait]
impl BamlTool for WeatherTool {
    const NAME: &'static str = "get_weather";
    
    fn description(&self) -> &'static str {
        "Gets the current weather for a specific location. Returns temperature, condition, and humidity."
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state or city and country, e.g. San Francisco, CA or London, UK"
                }
            },
            "required": ["location"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let obj = args.as_object().expect("Expected object");
        let location = obj.get("location")
            .and_then(|v| v.as_str())
            .expect("Expected 'location' string");
        
        tracing::info!(location = location, "WeatherTool executed");
        
        // Return mock weather data
        Ok(json!({
            "location": location,
            "temperature": "22°C",
            "temperature_f": 72,
            "condition": "Sunny with clear skies",
            "humidity": "65%",
            "wind_speed": "10 km/h",
            "description": format!("Current weather in {}: Sunny, 22°C, 65% humidity", location)
        }))
    }
}

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
                    "type": "string",
                    "description": "A mathematical expression to evaluate, e.g. '15 * 23' or '100 - 42'"
                }
            },
            "required": ["expression"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let obj = args.as_object().expect("Expected object");
        let expr = obj.get("expression")
            .and_then(|v| v.as_str())
            .expect("Expected 'expression' string");
        
        tracing::info!(expression = expr, "CalculatorTool executed");
        
        // Simple calculator implementation
        let result = if let Some(pos) = expr.find('+') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a + b
        } else if let Some(pos) = expr.find('-') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a - b
        } else if let Some(pos) = expr.find('*') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a * b
        } else if let Some(pos) = expr.find('/') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            if b != 0.0 { a / b } else { 0.0 }
        } else {
            0.0
        };
        
        Ok(json!({
            "expression": expr,
            "result": result,
            "formatted": format!("{} = {}", expr, result)
        }))
    }
}

/// Example uppercase string tool
pub struct UppercaseTool;

#[async_trait]
impl BamlTool for UppercaseTool {
    const NAME: &'static str = "uppercase";
    
    fn description(&self) -> &'static str {
        "Converts a string to uppercase"
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Text to convert to uppercase"}
            },
            "required": ["text"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let obj = args.as_object().expect("Expected object");
        let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({"result": text.to_uppercase(), "original": text}))
    }
}

