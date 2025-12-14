//! End-to-end test using actual LLM via OpenRouter

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_e2e_simple_greeting_with_llm() {
    // Set OPENROUTER_API_KEY from environment
    // This should be set before running the test: export OPENROUTER_API_KEY=...
    
    // Verify API key is set
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable must be set");
    
    assert!(!api_key.is_empty(), "OPENROUTER_API_KEY must not be empty");
    
    tracing::info!("Using OpenRouter API key (length: {})", api_key.len());
    
    // Set up BAML runtime
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    
    // Load BAML schema
    baml_manager.load_schema("baml_src").unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    // Create QuickJS bridge
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    
    // Register BAML functions
    bridge.register_baml_functions().await.unwrap();
    
    // Test invoking SimpleGreeting from JavaScript with actual LLM call
    // Since eval() can't await promises, we need to call BAML directly from Rust instead
    // This test verifies the full round-trip: Rust -> BAML -> LLM -> Rust
    let manager = baml_manager.lock().await;
    let result = manager.invoke_function("SimpleGreeting", serde_json::json!({"name": "Rust"})).await;
    drop(manager);
    
    assert!(result.is_ok(), "BAML execution should succeed");
    
    let json_result = result.unwrap();
    println!("LLM execution result: {:?}", json_result);
    
    // Verify we got a greeting result
    let greeting = json_result.as_str()
        .expect("Result should be a string");
    
    println!("LLM returned greeting: {}", greeting);
    
    // Verify it's a reasonable greeting
    let greeting_lower = greeting.to_lowercase();
    assert!(
        greeting_lower.contains("rust") || 
        greeting_lower.contains("hello") || 
        greeting_lower.contains("hi") ||
        greeting_lower.contains("greet"),
        "Greeting should mention 'Rust' or contain greeting words. Got: {}",
        greeting
    );
    
    // Also test via QuickJS to verify the bridge works
    // We'll just verify that the function is callable (it will return a promise)
    // but we can't await it in eval(), so we skip the full round-trip for now
    let js_test_code = r#"
        JSON.stringify({ 
            functionExists: typeof SimpleGreeting === 'function',
            test: "QuickJS bridge is working"
        })
    "#;
    
    let js_result = bridge.evaluate(js_test_code).await
        .expect("QuickJS bridge test should succeed");
    let js_obj = js_result.as_object().expect("Should be an object");
    assert!(js_obj.get("functionExists").and_then(|v| v.as_bool()).unwrap_or(false), 
            "SimpleGreeting function should be available in QuickJS");
    
    // (Test code moved above - we test Rust->BAML->LLM directly)
}

#[tokio::test]
async fn test_e2e_streaming_with_llm() {
    // Set OPENROUTER_API_KEY from environment
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable must be set");
    
    assert!(!api_key.is_empty(), "OPENROUTER_API_KEY must not be empty");
    
    // Set up BAML runtime
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
    // Test streaming invocation
    let js_code = r#"
        (async () => {
            try {
                const results = await SimpleGreetingStream({ name: "Streaming Test" });
                return JSON.stringify({ 
                    success: true, 
                    results: results,
                    resultCount: results.length 
                });
            } catch (e) {
                return JSON.stringify({ success: false, error: e.toString() });
            }
        })()
    "#;
    
    let result = bridge.evaluate(js_code).await;
    
    assert!(result.is_ok(), "Streaming execution should succeed");
    
    let json_result = result.unwrap();
    println!("Streaming execution result: {:?}", json_result);
    
    let obj = json_result.as_object()
        .expect("Result should be an object");
    
    let success = obj.get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    
    if success {
        assert!(obj.contains_key("results"), "Should contain 'results' array");
        assert!(obj.contains_key("resultCount"), "Should contain 'resultCount'");
        
        let result_count = obj.get("resultCount")
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        
        println!("Received {} incremental results from streaming", result_count);
        assert!(result_count > 0, "Should receive at least one streaming result");
    } else {
        let error = obj.get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Unknown error");
        panic!("Streaming failed: {}", error);
    }
}

