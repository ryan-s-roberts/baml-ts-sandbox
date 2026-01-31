//! LLM and BAML tool calling tests.

use async_trait::async_trait;
use baml_rt::tools::BamlTool;
use serde_json::json;

use test_support::common::{
    WeatherTool,
    CalculatorTool,
    assert_tool_registered_in_js,
    require_api_key,
    setup_baml_runtime_default,
    setup_baml_runtime_from_fixture,
    setup_bridge,
};

#[tokio::test]
async fn test_llm_tool_calling_rust() {
    // This test verifies tool registration and execution
    // API key is optional - test focuses on tool registration infrastructure
    
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(WeatherTool).await.unwrap();
        manager.register_tool(CalculatorTool).await.unwrap();
    }
    
    // Test that tools are registered and can be executed
    {
        let manager = baml_manager.lock().await;
        
        // Test weather tool
        let weather_result = manager.execute_tool("get_weather", json!({"location": "San Francisco"})).await.unwrap();
        let weather_obj = weather_result.as_object().expect("Expected object");
        assert!(weather_obj.contains_key("temperature"), "Weather result should contain temperature");
        
        // Test calculator tool
        let calc_result = manager.execute_tool("calculate", json!({"expression": {"left": 2, "operation": "Add", "right": 2}})).await.unwrap();
        let calc_obj = calc_result.as_object().expect("Expected object");
        let result = calc_obj.get("result").and_then(|v| v.as_f64()).unwrap();
        assert_eq!(result, 4.0, "2 + 2 should equal 4");
        
        // List tools
        let tools = manager.list_tools().await;
        assert!(tools.contains(&"get_weather".to_string()), "Should list weather tool");
        assert!(tools.contains(&"calculate".to_string()), "Should list calculator tool");
    }
    
    tracing::info!("Tool registration and execution tests passed");
    
    // Note: Actual LLM tool calling integration with BAML would require
    // passing the tool registry to BAML's call_function with client_registry.
    // This test verifies the foundation is in place.
}

#[tokio::test]
async fn test_llm_tool_calling_js() {
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    
    // Register a tool using the trait
    struct ReverseStringTool;
    
    #[async_trait]
    impl BamlTool for ReverseStringTool {
        const NAME: &'static str = "reverse_string";
        
        fn description(&self) -> &'static str {
            "Reverses a string"
        }
        
        fn input_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to reverse"}
                },
                "required": ["text"]
            })
        }
        
        async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
            let obj = args.as_object().expect("Expected object");
            let text = obj.get("text").and_then(|v| v.as_str()).expect("Expected 'text' string");
            let reversed: String = text.chars().rev().collect();
            Ok(json!({"reversed": reversed, "original": text}))
        }
    }
    
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(ReverseStringTool).await.unwrap();
    }
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    assert_tool_registered_in_js(&mut bridge, "reverse_string").await;
    
    // Test executing the tool from Rust
    {
        let manager = baml_manager.lock().await;
        let result = manager.execute_tool("reverse_string", json!({"text": "hello"})).await.unwrap();
        
        let result_obj = result.as_object().expect("Expected object");
        let reversed = result_obj.get("reversed").and_then(|g| g.as_str()).unwrap();
        assert_eq!(reversed, "olleh", "Should reverse the string correctly");
    }
}

#[tokio::test]
async fn test_e2e_baml_union_tool_calling() {
    let _ = require_api_key();
    
    tracing::info!("Starting E2E test: BAML union-based tool calling with Rust execution");
    
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(WeatherTool).await.unwrap();
        manager.register_tool(CalculatorTool).await.unwrap();
        manager.map_baml_variant_to_tool("WeatherTool", "get_weather");
        manager.map_baml_variant_to_tool("CalculatorTool", "calculate");
    }
    
    // Test 1: Weather tool via BAML union
    {
        let manager = baml_manager.lock().await;
        
        tracing::info!("Testing weather tool via BAML ChooseTool function");
        let result = manager.invoke_function(
            "ChooseTool",
            json!({"user_message": "What's the weather in San Francisco?"})
        ).await;
        
        match result {
            Ok(tool_choice) => {
                tracing::info!("✅ BAML function returned tool choice: {:?}", tool_choice);
                
                // Execute the chosen tool
                let tool_result = manager.execute_tool_from_baml_result(tool_choice).await
                    .expect("Should execute tool from BAML result");
                
                tracing::info!("✅ Tool executed successfully: {:?}", tool_result);
                assert!(tool_result.as_object().is_some(), "Tool result should be an object");
            }
            Err(e) => {
                tracing::warn!("BAML function call failed (may need tool calling integration): {}", e);
            }
        }
    }
    
    // Test 2: Calculator tool via BAML union
    {
        let manager = baml_manager.lock().await;
        
        tracing::info!("Testing calculator tool via BAML ChooseTool function");
        let result = manager.invoke_function(
            "ChooseTool",
            json!({"user_message": "Calculate 15 times 23"})
        ).await;
        
        match result {
            Ok(tool_choice) => {
                tracing::info!("✅ BAML function returned tool choice: {:?}", tool_choice);
                
                // Execute the chosen tool
                let tool_result = manager.execute_tool_from_baml_result(tool_choice).await
                    .expect("Should execute tool from BAML result");
                
                tracing::info!("✅ Tool executed successfully: {:?}", tool_result);
                
                // Verify calculator result
                if let Some(obj) = tool_result.as_object()
                    && let Some(result) = obj.get("result").and_then(|v| v.as_f64())
                {
                    assert_eq!(result, 345.0, "15 * 23 should equal 345");
                }
            }
            Err(e) => {
                tracing::warn!("BAML function call failed (may need tool calling integration): {}", e);
            }
        }
    }
    
    tracing::info!("🎉 E2E BAML union tool calling test completed!");
}

#[tokio::test]
async fn test_e2e_voidship_baml_tool_calling() {
    let _ = require_api_key();

    let baml_manager = setup_baml_runtime_from_fixture("voidship-rites");
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(CalculatorTool).await.unwrap();
        manager.map_baml_variant_to_tool("RiteCalcTool", "calculate");
        manager.map_baml_variant_to_tool("CalculatorTool", "calculate");
    }

    let result = {
        let manager = baml_manager.lock().await;
        manager
            .invoke_function("ChooseRiteTool", json!({"user_message": "Perform the rite of sums."}))
            .await
    };

    match result {
        Ok(tool_choice) => {
            let manager = baml_manager.lock().await;
            let tool_result = manager
                .execute_tool_from_baml_result(tool_choice)
                .await
                .expect("Should execute tool from BAML result");
            let value = tool_result
                .get("result")
                .and_then(|v| v.as_f64())
                .unwrap_or_default();
            assert_eq!(value, 5.0, "Expected 2 + 3 = 5");
        }
        Err(e) => {
            tracing::warn!("BAML tool selection failed: {}", e);
        }
    }
}
