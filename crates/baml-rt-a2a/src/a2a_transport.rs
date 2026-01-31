//! A2A request handler interface for non-standard transports.

use crate::a2a;
use crate::a2a_store::{TaskStore, TaskUpdateEvent};
use crate::a2a_types::{
    Artifact, CancelTaskRequest, GetTaskRequest, ListTasksRequest, ListTasksResponse,
    SendMessageResponse, Message, StreamResponse, SubscribeToTaskRequest, Task,
    TaskStatusUpdateEvent,
};
use baml_rt_quickjs::{BamlRuntimeManager, QuickJSBridge, QuickJSConfig};
use baml_rt_core::{BamlRtError, Result};
use baml_rt_core::correlation;
use baml_rt_observability::{metrics, spans};
use baml_rt_tools::{ToolExecutor, ToolMetadata};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

/// Top-level agent type that owns runtime, JS bridge, and A2A comms.
#[derive(Clone)]
pub struct A2aAgent {
    runtime: Arc<Mutex<BamlRuntimeManager>>,
    bridge: Arc<Mutex<QuickJSBridge>>,
    task_store: Arc<Mutex<TaskStore>>,
    update_tx: broadcast::Sender<TaskUpdateEvent>,
}

impl A2aAgent {
    /// Create a new agent with a runtime and JS bridge ready for A2A.
    pub async fn new() -> Result<Self> {
        A2aAgent::builder().build().await
    }

    /// Create a builder for configuring agent subcomponents.
    pub fn builder() -> A2aAgentBuilder {
        A2aAgentBuilder::new()
    }

    /// Access the underlying runtime manager.
    pub fn runtime(&self) -> Arc<Mutex<BamlRuntimeManager>> {
        self.runtime.clone()
    }

    /// Access the underlying JS bridge.
    pub fn bridge(&self) -> Arc<Mutex<QuickJSBridge>> {
        self.bridge.clone()
    }

    /// Access the task store for this agent instance.
    pub fn task_store(&self) -> Arc<Mutex<TaskStore>> {
        self.task_store.clone()
    }

    /// Subscribe to task update events for this agent instance.
    pub fn subscribe_task_updates(&self) -> broadcast::Receiver<TaskUpdateEvent> {
        self.update_tx.subscribe()
    }

    /// Evaluate JavaScript in the agent runtime.
    pub async fn evaluate_js(&self, code: &str) -> Result<Value> {
        let mut bridge = self.bridge.lock().await;
        bridge.evaluate(code).await
    }

    /// Register a JavaScript tool and expose it to BAML-native tool calls.
    pub async fn register_js_tool(
        &self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        js_function_code: impl AsRef<str>,
    ) -> Result<()> {
        let name = name.into();
        {
            let mut bridge = self.bridge.lock().await;
            bridge.register_js_tool(&name, js_function_code).await?;
        }

        let metadata = ToolMetadata {
            name: name.clone(),
            description: description.into(),
            input_schema,
        };

        let executor: Arc<dyn ToolExecutor> = Arc::new(JsToolExecutor {
            bridge: self.bridge.clone(),
            tool_name: name,
        });

        let registry = {
            let runtime = self.runtime.lock().await;
            runtime.tool_registry()
        };
        let mut registry = registry.lock().await;
        registry.register_dynamic(metadata, executor)?;

        Ok(())
    }
}

/// Builder for configuring an A2A agent and its subcomponents.
pub struct A2aAgentBuilder {
    runtime: Option<Arc<Mutex<BamlRuntimeManager>>>,
    bridge: Option<Arc<Mutex<QuickJSBridge>>>,
    quickjs_config: QuickJSConfig,
    register_baml_functions: bool,
    init_js: Vec<String>,
}

impl A2aAgentBuilder {
    pub fn new() -> Self {
        Self {
            runtime: None,
            bridge: None,
            quickjs_config: QuickJSConfig::default(),
            register_baml_functions: true,
            init_js: Vec::new(),
        }
    }

    /// Provide an existing runtime manager.
    pub fn with_runtime_manager(mut self, runtime: BamlRuntimeManager) -> Self {
        self.runtime = Some(Arc::new(Mutex::new(runtime)));
        self
    }

    /// Provide a shared runtime manager.
    pub fn with_runtime_handle(mut self, runtime: Arc<Mutex<BamlRuntimeManager>>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Provide a shared QuickJS bridge (requires a runtime handle too).
    pub fn with_bridge_handle(mut self, bridge: Arc<Mutex<QuickJSBridge>>) -> Self {
        self.bridge = Some(bridge);
        self
    }

    /// Configure QuickJS runtime options used when creating the bridge.
    pub fn with_quickjs_config(mut self, config: QuickJSConfig) -> Self {
        self.quickjs_config = config;
        self
    }

    /// Enable or disable registration of BAML helper functions.
    pub fn with_baml_helpers(mut self, enabled: bool) -> Self {
        self.register_baml_functions = enabled;
        self
    }

    /// Add JavaScript to evaluate after the bridge is created.
    pub fn with_init_js(mut self, code: impl Into<String>) -> Self {
        self.init_js.push(code.into());
        self
    }

    /// Build the agent with the configured subcomponents.
    pub async fn build(self) -> Result<A2aAgent> {
        if self.bridge.is_some() && self.runtime.is_none() {
            return Err(BamlRtError::InvalidArgument(
                "A2aAgentBuilder requires a runtime handle when providing a bridge".to_string(),
            ));
        }

        let runtime = match self.runtime {
            Some(runtime) => runtime,
            None => Arc::new(Mutex::new(BamlRuntimeManager::new()?)),
        };

        let bridge = match self.bridge {
            Some(bridge) => bridge,
            None => {
                let bridge =
                    QuickJSBridge::new_with_config(runtime.clone(), self.quickjs_config).await?;
                Arc::new(Mutex::new(bridge))
            }
        };

        if self.register_baml_functions || !self.init_js.is_empty() {
            let mut bridge_guard = bridge.lock().await;
            if self.register_baml_functions {
                bridge_guard.register_baml_functions().await?;
            }
            for code in self.init_js {
                bridge_guard.evaluate(&code).await?;
            }
        }

        let (update_tx, _update_rx) = broadcast::channel(256);
        Ok(A2aAgent {
            runtime,
            bridge,
            task_store: Arc::new(Mutex::new(TaskStore::new())),
            update_tx,
        })
    }
}

impl Default for A2aAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for alternative, non-standard A2A transports.
///
/// The transport receives raw JSON and returns JSON-RPC responses.
#[async_trait(?Send)]
pub trait A2aRequestHandler: Send + Sync {
    async fn handle_a2a(&self, request: Value) -> Result<Vec<Value>>;
}

#[async_trait(?Send)]
impl A2aRequestHandler for A2aAgent {
    async fn handle_a2a(&self, request: Value) -> Result<Vec<Value>> {
        let request_id = a2a::extract_jsonrpc_id(&request);
        let parsed_request = match a2a::A2aRequest::from_value(request) {
            Ok(parsed) => parsed,
            Err(err) => {
                let (code, message, data) = map_jsonrpc_error(&err);
                return Ok(vec![a2a::error_response(request_id, code, message, data)]);
            }
        };
        let correlation_id = parsed_request
            .correlation_id()
            .unwrap_or_else(correlation::generate_correlation_id);

        let span = if parsed_request.is_stream {
            spans::a2a_stream(parsed_request.method.as_str(), &correlation_id)
        } else {
            spans::a2a_request(parsed_request.method.as_str(), &correlation_id)
        };
        let _guard = span.enter();
        let start = std::time::Instant::now();
        let method = parsed_request.method;
        let is_stream = parsed_request.is_stream;

        let outcome = correlation::with_correlation_id(correlation_id, async move {
            match parsed_request.method {
                a2a::A2aMethod::TasksGet => self.handle_tasks_get(parsed_request.params).await,
                a2a::A2aMethod::TasksList => self.handle_tasks_list(parsed_request.params).await,
                a2a::A2aMethod::TasksCancel => self.handle_tasks_cancel(parsed_request.params).await,
                a2a::A2aMethod::TasksSubscribe => self
                    .handle_tasks_subscribe(parsed_request.params, parsed_request.is_stream)
                    .await,
                _ => {
                    if parsed_request.is_stream {
                        let chunks = self.invoke_a2a_stream(&parsed_request).await?;
                        for chunk in &chunks {
                            self.store_a2a_result(chunk).await?;
                        }
                        Ok(a2a::A2aOutcome::Stream(chunks))
                    } else {
                        let result = self.invoke_a2a_handler(&parsed_request).await?;
                        self.store_a2a_result(&result).await?;
                        Ok(a2a::A2aOutcome::Response(result))
                    }
                }
            }
        })
        .await;

        let duration = start.elapsed();
        match &outcome {
            Ok(a2a::A2aOutcome::Stream(chunks)) => {
                metrics::record_a2a_request(method.as_str(), "success", is_stream, duration);
                metrics::record_a2a_stream_chunks(method.as_str(), chunks.len());
            }
            Ok(_) => metrics::record_a2a_request(method.as_str(), "success", is_stream, duration),
            Err(err) => {
                metrics::record_a2a_request(method.as_str(), "error", is_stream, duration);
                metrics::record_a2a_error(method.as_str(), classify_a2a_error(err), is_stream);
            }
        }

        let responses = match outcome {
            Ok(a2a::A2aOutcome::Response(result)) => {
                vec![a2a::success_response(request_id, result)]
            }
            Ok(a2a::A2aOutcome::Stream(chunks)) => {
                let total = chunks.len();
                let mut responses = Vec::with_capacity(total);
                for (idx, chunk) in chunks.into_iter().enumerate() {
                    responses.push(a2a::stream_chunk_response(
                        request_id.clone(),
                        chunk,
                        idx,
                        idx + 1 == total,
                    ));
                }
                responses
            }
            Err(err) => {
                let (code, message, data) = map_jsonrpc_error(&err);
                vec![a2a::error_response(request_id, code, message, data)]
            }
        };

        Ok(responses)
    }
}

impl A2aAgent {
    async fn invoke_a2a_handler(&self, request: &a2a::A2aRequest) -> Result<Value> {
        let js_request = a2a::request_to_js_value(request);
        let mut bridge = self.bridge.lock().await;
        bridge.invoke_js_function("handle_a2a_request", js_request).await
    }

    async fn invoke_a2a_stream(&self, request: &a2a::A2aRequest) -> Result<Vec<Value>> {
        let result = self.invoke_a2a_handler(request).await?;
        match result {
            Value::Array(values) => values
                .into_iter()
                .map(normalize_stream_chunk)
                .collect::<Result<Vec<Value>>>(),
            Value::Object(map) if map.get("error").is_some() => Err(BamlRtError::QuickJs(
                map.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            )),
            other => Ok(vec![normalize_stream_chunk(other)?]),
        }
    }

    async fn store_a2a_result(&self, value: &Value) -> Result<()> {
        if (value.get("statusUpdate").is_some() || value.get("artifactUpdate").is_some())
            && let Ok(stream) = serde_json::from_value::<StreamResponse>(value.clone())
        {
            let mut store = self.task_store.lock().await;
            if let Some(task) = stream.task {
                let status = task.status.clone();
                let context_id = task.context_id.clone();
                let task_id = task.id.clone();
                let artifacts = task.artifacts.clone();
                store.upsert(task);
                if let Some(status) = status
                    && let Some(event) = store.record_status_update(
                        task_id.clone(),
                        context_id.clone(),
                        status,
                    )
                {
                    self.emit_update(event);
                }
                if let Some(task_id) = task_id {
                    self.emit_artifact_updates(&mut store, &task_id, context_id, artifacts);
                }
            }
            if let Some(message) = stream.message {
                store.insert_message(&message);
            }
            if let Some(TaskStatusUpdateEvent {
                task_id,
                context_id,
                status: Some(status),
                ..
            }) = stream.status_update
                && let Some(event) = store.record_status_update(task_id, context_id, status)
            {
                self.emit_update(event);
            }
            if let Some(artifact_update) = stream.artifact_update
                && let Some(event) = store.record_artifact_update(
                    artifact_update.task_id,
                    artifact_update.context_id,
                    artifact_update.artifact.unwrap_or_default(),
                    artifact_update.append,
                    artifact_update.last_chunk,
                )
            {
                self.emit_update(event);
            }
            return Ok(());
        }

        if let Ok(response) = serde_json::from_value::<SendMessageResponse>(value.clone()) {
            let mut store = self.task_store.lock().await;
            if let Some(task) = response.task {
                let status = task.status.clone();
                let context_id = task.context_id.clone();
                let task_id = task.id.clone();
                let artifacts = task.artifacts.clone();
                store.upsert(task);
                if let Some(status) = status
                    && let Some(event) = store.record_status_update(
                        task_id.clone(),
                        context_id.clone(),
                        status,
                    )
                {
                    self.emit_update(event);
                }
                if let Some(task_id) = task_id {
                    self.emit_artifact_updates(&mut store, &task_id, context_id, artifacts);
                }
            }
            if let Some(message) = response.message {
                store.insert_message(&message);
            }
            return Ok(());
        }

        if let Ok(stream) = serde_json::from_value::<StreamResponse>(value.clone()) {
            let mut store = self.task_store.lock().await;
            if let Some(task) = stream.task {
                let status = task.status.clone();
                let context_id = task.context_id.clone();
                let task_id = task.id.clone();
                let artifacts = task.artifacts.clone();
                store.upsert(task);
                if let Some(status) = status
                    && let Some(event) = store.record_status_update(
                        task_id.clone(),
                        context_id.clone(),
                        status,
                    )
                {
                    self.emit_update(event);
                }
                if let Some(task_id) = task_id {
                    self.emit_artifact_updates(&mut store, &task_id, context_id, artifacts);
                }
            }
            if let Some(message) = stream.message {
                store.insert_message(&message);
            }
            if let Some(TaskStatusUpdateEvent {
                task_id,
                context_id,
                status: Some(status),
                ..
            }) = stream.status_update
                && let Some(event) = store.record_status_update(task_id, context_id, status)
            {
                self.emit_update(event);
            }
            if let Some(artifact_update) = stream.artifact_update
                && let Some(event) = store.record_artifact_update(
                    artifact_update.task_id,
                    artifact_update.context_id,
                    artifact_update.artifact.unwrap_or_default(),
                    artifact_update.append,
                    artifact_update.last_chunk,
                )
            {
                self.emit_update(event);
            }
            return Ok(());
        }

        if let Ok(task) = serde_json::from_value::<Task>(value.clone()) {
            let mut store = self.task_store.lock().await;
            let status = task.status.clone();
            let context_id = task.context_id.clone();
            let task_id = task.id.clone();
            let artifacts = task.artifacts.clone();
            store.upsert(task);
            if let Some(status) = status
                && let Some(event) = store.record_status_update(
                    task_id.clone(),
                    context_id.clone(),
                    status,
                )
            {
                self.emit_update(event);
            }
            if let Some(task_id) = task_id {
                self.emit_artifact_updates(&mut store, &task_id, context_id, artifacts);
            }
            return Ok(());
        }

        Ok(())
    }

    async fn handle_tasks_get(&self, params: Value) -> Result<a2a::A2aOutcome> {
        let request: GetTaskRequest = serde_json::from_value(params).map_err(BamlRtError::Json)?;
        let store = self.task_store.lock().await;
        let history_length = request.history_length.and_then(|value| value.as_usize());
        let task = store
            .get(&request.id, history_length)
            .ok_or_else(|| BamlRtError::InvalidArgument("Task not found".to_string()))?;
        let value = serde_json::to_value(task).map_err(BamlRtError::Json)?;
        Ok(a2a::A2aOutcome::Response(value))
    }

    async fn handle_tasks_list(&self, params: Value) -> Result<a2a::A2aOutcome> {
        let request: ListTasksRequest = serde_json::from_value(params).map_err(BamlRtError::Json)?;
        let store = self.task_store.lock().await;
        let response: ListTasksResponse = store.list(&request);
        let value = serde_json::to_value(response).map_err(BamlRtError::Json)?;
        Ok(a2a::A2aOutcome::Response(value))
    }

    async fn handle_tasks_cancel(&self, params: Value) -> Result<a2a::A2aOutcome> {
        let request: CancelTaskRequest =
            serde_json::from_value(params).map_err(BamlRtError::Json)?;
        let task = {
            let mut store = self.task_store.lock().await;
            let task = store
                .cancel(&request.id)
                .ok_or_else(|| BamlRtError::InvalidArgument("Task not found".to_string()))?;
            if let Some(status) = task.status.clone()
                && let Some(event) = store.record_status_update(
                    task.id.clone(),
                    task.context_id.clone(),
                    status,
                )
            {
                self.emit_update(event);
            }
            task
        };

        {
            let mut bridge = self.bridge.lock().await;
            let _ = bridge
                .invoke_optional_js_function(
                    "handle_a2a_cancel",
                    serde_json::to_value(&request).map_err(BamlRtError::Json)?,
                )
                .await?;
        }

        let value = serde_json::to_value(task).map_err(BamlRtError::Json)?;
        Ok(a2a::A2aOutcome::Response(value))
    }

    async fn handle_tasks_subscribe(
        &self,
        params: Value,
        is_stream: bool,
    ) -> Result<a2a::A2aOutcome> {
        let request: SubscribeToTaskRequest =
            serde_json::from_value(params).map_err(BamlRtError::Json)?;
        let mut store = self.task_store.lock().await;
        let task = store
            .get(&request.id, None)
            .ok_or_else(|| BamlRtError::InvalidArgument("Task not found".to_string()))?;
        let value = serde_json::to_value(&task).map_err(BamlRtError::Json)?;

        if is_stream {
            let mut responses = Vec::new();
            let status_update = task.status.as_ref().map(|status| TaskStatusUpdateEvent {
                context_id: task.context_id.clone(),
                task_id: task.id.clone(),
                status: Some(status.clone()),
                metadata: None,
                extra: HashMap::new(),
            });
            let response = StreamResponse {
                task: Some(task),
                status_update,
                message: None,
                artifact_update: None,
                extra: HashMap::new(),
            };
            responses.push(serde_json::to_value(response).map_err(BamlRtError::Json)?);

            for update in store.drain_updates(&request.id) {
                let stream_response = match update {
                    TaskUpdateEvent::Status(status_update) => StreamResponse {
                        status_update: Some(status_update),
                        message: None,
                        task: None,
                        artifact_update: None,
                        extra: HashMap::new(),
                    },
                    TaskUpdateEvent::Artifact(artifact_update) => StreamResponse {
                        artifact_update: Some(artifact_update),
                        message: None,
                        task: None,
                        status_update: None,
                        extra: HashMap::new(),
                    },
                };
                responses.push(serde_json::to_value(stream_response).map_err(BamlRtError::Json)?);
            }

            Ok(a2a::A2aOutcome::Stream(responses))
        } else {
            Ok(a2a::A2aOutcome::Response(value))
        }
    }
}

impl A2aAgent {
    fn emit_update(&self, update: TaskUpdateEvent) {
        let _ = self.update_tx.send(update);
    }

    fn emit_artifact_updates(
        &self,
        store: &mut TaskStore,
        task_id: &str,
        context_id: Option<String>,
        artifacts: Vec<Artifact>,
    ) {
        for artifact in artifacts {
            if let Some(event) = store.record_artifact_update(
                Some(task_id.to_string()),
                context_id.clone(),
                artifact,
                Some(false),
                Some(true),
            ) {
                self.emit_update(event);
            }
        }
    }
}

fn normalize_stream_chunk(value: Value) -> Result<Value> {
    if is_stream_response(&value) {
        return Ok(value);
    }
    if let Ok(message) = serde_json::from_value::<Message>(value.clone()) {
        let response = StreamResponse {
            message: Some(message),
            task: None,
            status_update: None,
            artifact_update: None,
            extra: HashMap::new(),
        };
        return serde_json::to_value(response).map_err(BamlRtError::Json);
    }
    if let Ok(task) = serde_json::from_value::<Task>(value.clone()) {
        let response = StreamResponse {
            task: Some(task),
            message: None,
            status_update: None,
            artifact_update: None,
            extra: HashMap::new(),
        };
        return serde_json::to_value(response).map_err(BamlRtError::Json);
    }
    Ok(value)
}

fn is_stream_response(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.contains_key("message")
        || map.contains_key("task")
        || map.contains_key("statusUpdate")
        || map.contains_key("artifactUpdate")
}

fn classify_a2a_error(error: &BamlRtError) -> &'static str {
    match error {
        BamlRtError::InvalidArgument(_) => "invalid_argument",
        BamlRtError::FunctionNotFound(_) => "function_not_found",
        BamlRtError::QuickJs(_) => "quickjs",
        BamlRtError::Json(_) => "json",
        BamlRtError::ToolExecution(_) => "tool_execution",
        _ => "internal",
    }
}

struct JsToolExecutor {
    bridge: Arc<Mutex<QuickJSBridge>>,
    tool_name: String,
}

#[async_trait]
impl ToolExecutor for JsToolExecutor {
    async fn execute(&self, args: Value) -> Result<Value> {
        let bridge = self.bridge.clone();
        let tool_name = self.tool_name.clone();
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            handle.block_on(async move {
                let mut bridge = bridge.lock().await;
                let result = bridge.invoke_js_tool(&tool_name, args).await?;
                if let Some(error) = result.get("error").and_then(Value::as_str) {
                    return Err(BamlRtError::QuickJs(error.to_string()));
                }
                Ok(result)
            })
        })
        .await
        .map_err(|err| BamlRtError::QuickJsWithSource {
            context: "js tool join error".to_string(),
            source: Box::new(err),
        })?
    }
}

fn map_jsonrpc_error(error: &BamlRtError) -> (i64, &'static str, Option<Value>) {
    match error {
        BamlRtError::InvalidArgument(message) => (
            -32600,
            "Invalid request",
            Some(json!({
                "error": error.to_string(),
                "details": message,
            })),
        ),
        BamlRtError::FunctionNotFound(name) => (
            -32601,
            "Method not found",
            Some(json!({
                "error": error.to_string(),
                "function": name,
            })),
        ),
        BamlRtError::Json(json_err) => (
            -32700,
            "Parse error",
            Some(json!({
                "error": error.to_string(),
                "details": json_err.to_string(),
            })),
        ),
        BamlRtError::QuickJsWithSource { context, .. } => (
            -32603,
            "Internal error",
            Some(json!({
                "error": error.to_string(),
                "context": context,
            })),
        ),
        _ => (
            -32603,
            "Internal error",
            Some(json!({"error": error.to_string()})),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::A2aAgent;
    use serde_json::json;

    #[tokio::test]
    async fn js_tool_can_be_called_via_baml_tool_registry() {
        let agent = A2aAgent::builder().build().await.expect("agent build");

        agent
            .register_js_tool(
                "add_js",
                "Adds two numbers",
                json!({
                    "type": "object",
                    "properties": {
                        "a": {"type": "number"},
                        "b": {"type": "number"}
                    },
                    "required": ["a", "b"]
                }),
                r#"(args) => ({ sum: args.a + args.b })"#,
            )
            .await
            .expect("register js tool");

        let runtime = agent.runtime();
        let result = {
            let runtime = runtime.lock().await;
            runtime
                .execute_tool("add_js", json!({"a": 2, "b": 3}))
                .await
                .expect("execute tool")
        };

        assert_eq!(result.get("sum").and_then(|v| v.as_i64()), Some(5));
    }
}
