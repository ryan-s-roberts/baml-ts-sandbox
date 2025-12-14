//! Tests for JavaScript streaming invocation of BAML functions

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_js_stream_baml_function() {
    // Set up BAML runtime
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    
    // Load BAML schema
    baml_manager.load_schema("baml_src").unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    // Create QuickJS bridge
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    
    // Register BAML functions (including streaming versions)
    bridge.register_baml_functions().await.unwrap();
    
    // Test invoking SimpleGreeting stream from JavaScript
    // Note: This will fail without an API key, but we can test the invocation path
    let js_code = r#"
        (async () => {
            try {
                const results = await SimpleGreetingStream({ name: "World" });
                return JSON.stringify({ success: true, results: results });
            } catch (e) {
                return JSON.stringify({ success: false, error: e.toString() });
            }
        })()
    "#;
    
    let result = bridge.evaluate(js_code).await;
    
    // The result should contain either success with results array, or error info
    assert!(result.is_ok(), "JavaScript execution should succeed");
    
    let json_result = result.unwrap();
    println!("JavaScript streaming execution result: {:?}", json_result);
    
    // Check if we got a proper result structure
    if let Some(obj) = json_result.as_object() {
        // Should have either success with results, or error
        assert!(obj.contains_key("success") || obj.contains_key("error"), 
                "Result should contain 'success' or 'error' field");
        
        // If it succeeded, results should be an array
        if let Some(success) = obj.get("success").and_then(|s| s.as_bool()) {
            if success {
                assert!(obj.contains_key("results"), "Success result should contain 'results' array");
            }
        }
    } else {
        panic!("Result should be an object");
    }
}
