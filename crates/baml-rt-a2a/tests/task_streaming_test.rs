use test_support::common;
use async_trait::async_trait;
use baml_rt::a2a_types::{JSONRPCId, JSONRPCRequest, Message, MessageRole, Part, SendMessageRequest};
use baml_rt::tools::BamlTool;
use baml_rt::baml::BamlRuntimeManager;
use baml_rt::{A2aAgent, A2aRequestHandler};
use serde_json::{json, Value};
use std::fs;
use std::collections::HashMap;
use test_support::common::CalculatorTool;

fn fixture_agent_dir() -> std::path::PathBuf {
    common::agent_fixture("voidship-rites")
}

fn fixture_js_code() -> String {
    let agent_dir = fixture_agent_dir();
    let dist_path = agent_dir.join("dist").join("index.js");
    if dist_path.exists() {
        return fs::read_to_string(&dist_path).expect("fixture JS should be readable");
    }
    let src_path = agent_dir.join("src").join("index.ts");
    fs::read_to_string(&src_path).expect("fixture JS should be readable")
}

fn user_message(message_id: &str, text: &str) -> Message {
    Message {
        message_id: message_id.to_string(),
        role: MessageRole::String("ROLE_USER".to_string()),
        parts: vec![Part {
            text: Some(text.to_string()),
            ..Part::default()
        }],
        context_id: Some("ctx-void-001".to_string()),
        task_id: None,
        reference_task_ids: Vec::new(),
        extensions: Vec::new(),
        metadata: None,
        extra: HashMap::new(),
    }
}

async fn setup_agent() -> A2aAgent {
    let mut manager = BamlRuntimeManager::new().unwrap();
    let agent_dir = fixture_agent_dir();
    manager.load_schema(agent_dir.to_str().unwrap()).unwrap();
    manager.map_baml_variant_to_tool("CalculatorTool", "calculate");
    A2aAgent::builder()
        .with_runtime_manager(manager)
        .with_init_js(fixture_js_code())
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn test_message_send_deterministic_task() {
    let agent = setup_agent().await;
    let params = SendMessageRequest {
        message: user_message("vox-1", "long-rite: reactor benediction"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.send".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
        id: Some(JSONRPCId::String("req-1".to_string())),
    };

    let responses = agent
        .handle_a2a(serde_json::to_value(request).unwrap())
        .await
        .unwrap();
    let result = responses[0].get("result").cloned().unwrap_or(Value::Null);
    let task_id = result
        .get("task")
        .and_then(|task| task.get("id"))
        .and_then(|value| value.as_str());
    assert_eq!(task_id, Some("rite-task-vox-1"));
}

#[tokio::test]
async fn test_message_send_stream_emits_updates() {
    let agent = setup_agent().await;
    let params = SendMessageRequest {
        message: user_message("vox-2", "ignite the void seals"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.sendStream".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
        id: Some(JSONRPCId::String("req-2".to_string())),
    };

    let responses = agent
        .handle_a2a(serde_json::to_value(request).unwrap())
        .await
        .unwrap();

    let mut saw_status = false;
    let mut saw_artifact = false;
    for response in responses {
        if let Some(chunk) = response
            .get("result")
            .and_then(|result| result.get("chunk"))
        {
            if chunk.get("statusUpdate").is_some() {
                saw_status = true;
            }
            if chunk.get("artifactUpdate").is_some() {
                saw_artifact = true;
            }
        }
    }

    assert!(saw_status, "expected a statusUpdate stream chunk");
    assert!(saw_artifact, "expected an artifactUpdate stream chunk");
}

#[tokio::test]
async fn test_tasks_subscribe_streams_incremental_updates() {
    let agent = setup_agent().await;
    let params = SendMessageRequest {
        message: user_message("vox-3", "long-rite: plasma canticle"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let create_request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.send".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
        id: Some(JSONRPCId::String("req-3".to_string())),
    };
    let _ = agent
        .handle_a2a(serde_json::to_value(create_request).unwrap())
        .await
        .unwrap();

    let stream_params = SendMessageRequest {
        message: user_message("vox-3", "ignite the void seals"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let stream_request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.sendStream".to_string(),
        params: Some(serde_json::to_value(stream_params).unwrap()),
        id: Some(JSONRPCId::String("req-4".to_string())),
    };
    let _ = agent
        .handle_a2a(serde_json::to_value(stream_request).unwrap())
        .await
        .unwrap();

    let subscribe_request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "tasks.subscribe".to_string(),
        params: Some(json!({ "id": "rite-task-vox-3", "stream": true })),
        id: Some(JSONRPCId::String("req-5".to_string())),
    };
    let responses = agent
        .handle_a2a(serde_json::to_value(subscribe_request).unwrap())
        .await
        .unwrap();

    let mut saw_status = false;
    let mut saw_artifact = false;
    for response in responses {
        if let Some(chunk) = response
            .get("result")
            .and_then(|result| result.get("chunk"))
        {
            if chunk.get("statusUpdate").is_some() {
                saw_status = true;
            }
            if chunk.get("artifactUpdate").is_some() {
                saw_artifact = true;
            }
        }
    }

    assert!(saw_status, "expected status updates in subscribe stream");
    assert!(saw_artifact, "expected artifact updates in subscribe stream");
}

struct AddNumbersTool;

#[async_trait]
impl BamlTool for AddNumbersTool {
    const NAME: &'static str = "add_numbers";

    fn description(&self) -> &'static str {
        "Adds two numbers together"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let a = obj.get("a").and_then(|v| v.as_f64()).expect("Expected 'a' number");
        let b = obj.get("b").and_then(|v| v.as_f64()).expect("Expected 'b' number");
        Ok(json!({ "result": a + b }))
    }
}

#[tokio::test]
async fn test_message_send_tool_calling() {
    let agent = setup_agent().await;
    {
        let runtime = agent.runtime();
        let mut manager = runtime.lock().await;
        manager.register_tool(AddNumbersTool).await.unwrap();
    }

    let params = SendMessageRequest {
        message: user_message("vox-4", "tool-call: add numbers"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.send".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
        id: Some(JSONRPCId::String("req-6".to_string())),
    };

    let responses = agent
        .handle_a2a(serde_json::to_value(request).unwrap())
        .await
        .unwrap();
    let result = responses[0].get("result").cloned().unwrap_or(Value::Null);
    let text = result
        .get("message")
        .and_then(|message| message.get("parts"))
        .and_then(|parts| parts.as_array())
        .and_then(|parts| parts.first())
        .and_then(|part| part.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert!(
        text.contains("sum=5"),
        "expected tool result in message text, got: {}",
        text
    );
}

#[tokio::test]
async fn test_message_send_baml_tool_calling() {
    let agent = setup_agent().await;
    {
        let runtime = agent.runtime();
        let mut manager = runtime.lock().await;
        manager.register_tool(CalculatorTool).await.unwrap();
        manager.map_baml_variant_to_tool("RiteCalcTool", "calculate");
    }

    let params = SendMessageRequest {
        message: user_message("vox-5", "baml-tool: rite of sums"),
        configuration: None,
        metadata: None,
        tenant: None,
        extra: HashMap::new(),
    };
    let request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "message.send".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
        id: Some(JSONRPCId::String("req-7".to_string())),
    };

    let responses = agent
        .handle_a2a(serde_json::to_value(request).unwrap())
        .await
        .unwrap();
    let result = responses[0].get("result").cloned().unwrap_or(Value::Null);
    let text = result
        .get("message")
        .and_then(|message| message.get("parts"))
        .and_then(|parts| parts.as_array())
        .and_then(|parts| parts.first())
        .and_then(|part| part.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert!(
        text.contains("sum=5"),
        "expected BAML tool result in message text, got: {}",
        text
    );
}
