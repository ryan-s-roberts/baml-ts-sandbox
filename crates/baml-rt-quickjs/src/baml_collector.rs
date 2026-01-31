//! BAML Collector implementation for LLM call interception
//!
//! This module implements a collector that hooks into BAML's execution lifecycle
//! to intercept LLM calls and route them through our interceptor system.
//!
//! BAML Collector implementation for LLM call interception
//!
//! This module implements a collector that hooks into BAML's execution lifecycle
//! to intercept LLM calls and route them through our interceptor system.

use baml_rt_core::Result;
use baml_rt_interceptor::{InterceptorRegistry, LLMCallContext};
use baml_runtime::tracingv2::storage::storage::Collector;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

/// BAML collector wrapper that tracks LLM calls via trace events
///
/// This wraps BAML's Collector to track function execution and extract
/// LLM call information from trace events for interceptor notifications.
pub struct BamlLLMCollector {
    inner: Arc<Collector>,
    interceptor_registry: Arc<Mutex<InterceptorRegistry>>,
    function_name: String,
}

impl BamlLLMCollector {
    /// Create a new BAML LLM collector
    pub fn new(
        interceptor_registry: Arc<Mutex<InterceptorRegistry>>,
        function_name: String,
    ) -> Self {
        let inner = Arc::new(Collector::new(Some(format!("llm_interceptor_{}", function_name))));
        Self {
            inner,
            interceptor_registry,
            function_name,
        }
    }

    /// Get a reference to the inner BAML Collector
    pub fn as_collector(&self) -> Arc<Collector> {
        self.inner.clone()
    }

    /// Track a function call ID so we can later process its trace events
    pub fn track_function_call(&self, _function_id: impl Clone + Send + Sync + 'static) {
        // The collector will automatically track this when we pass it to call_function
        // But we expose this method for manual tracking if needed
    }

    /// Process trace events to extract LLM call information and notify interceptors
    ///
    /// This should be called after function execution to process collected trace events.
    ///
    /// Note: This uses the last function log tracked by the collector.
    pub async fn process_trace_events(&self) -> Result<()> {

        // Get the last function log tracked by this collector
        // The collector tracks function IDs as they're executed when passed to call_function
        let mut function_log = match self.inner.last_function_log() {
            Some(log) => log,
            None => {
                // No function log found - this is fine, just means no LLM calls were made
                // or the function didn't trigger any LLM calls
                return Ok(());
            }
        };

        // Extract LLM calls from the function log
        let llm_calls = function_log.calls();

        // Process each LLM call and notify interceptors
        for call_kind in llm_calls {
            // Extract context from the LLM call
            if let Some(llm_call) = call_kind.as_request() {
                let context = self.extract_context_from_llm_call(llm_call);

                // Extract duration from timing
                let duration_ms = llm_call.timing.duration_ms.unwrap_or(0) as u64;

                // Notify interceptors (post-execution notification)
                let registry = self.interceptor_registry.lock().await;
                // For post-execution, we just notify of completion
                let result: Result<serde_json::Value> = Ok(serde_json::to_value(llm_call)
                    .unwrap_or_else(|_| json!({})));
                registry.notify_llm_call_complete(&context, &result, duration_ms).await;
            }
            // TODO: Handle stream calls (call_kind.as_stream())
        }

        Ok(())
    }

    /// Extract LLM call context from an LLMCall
    fn extract_context_from_llm_call(&self, call: &baml_runtime::tracingv2::storage::storage::LLMCall) -> LLMCallContext {
        // Extract client/provider from the call
        let client = call.client_name.clone();
        let model = call.provider.clone(); // provider is the model/provider name

        // Extract prompt/messages from the request if available
        let prompt = if let Some(ref http_request) = call.request {
            serde_json::to_value(http_request.as_ref())
                .unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        LLMCallContext {
            client,
            model,
            function_name: self.function_name.clone(),
            prompt,
            metadata: json!({
                "usage": call.usage,
                "selected": call.selected,
            }),
        }
    }
}
