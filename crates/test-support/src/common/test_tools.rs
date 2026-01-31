//! Test tool implementations used across test suites.

use async_trait::async_trait;
use baml_rt::Result;
use baml_rt::tools::BamlTool;
use serde_json::{json, Value};

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
        let location = obj
            .get("location")
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

/// Delayed response tool for testing async operations
pub struct DelayedResponseTool;

#[async_trait]
impl BamlTool for DelayedResponseTool {
    const NAME: &'static str = "delayed_response";
    
    fn description(&self) -> &'static str {
        "Returns a response after a short delay (simulates async operation)"
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Message to return"}
            },
            "required": ["message"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        use tokio::time::{sleep, Duration};
        
        let obj = args.as_object().expect("Expected object");
        let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");
        
        // Simulate async work
        sleep(Duration::from_millis(50)).await;
        
        Ok(json!({
            "response": format!("Delayed: {}", message),
            "timestamp": format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
        }))
    }
}
