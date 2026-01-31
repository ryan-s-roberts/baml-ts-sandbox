//! BAML Agent Runner
//!
//! This binary loads and executes one or more packaged agent applications.
//! Each agent package is a tar.gz containing BAML schemas, compiled TypeScript,
//! and metadata.

use baml_rt_a2a::{A2aAgent, A2aRequestHandler, a2a};
use baml_rt_a2a::a2a_types::JSONRPCId;
use baml_rt_core::{BamlRtError, Result};
use baml_rt_observability::{spans, tracing_setup};
use baml_rt_quickjs::{BamlRuntimeManager, QuickJSBridge};
use anyhow::Context;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Agent package metadata
#[derive(Debug, Clone)]
struct AgentManifest {
    version: String,
    name: String,
    entry_point: String,
}

/// Agent package loader and executor
struct AgentPackage {
    name: String,
    agent: A2aAgent,
}

impl AgentPackage {
    /// Load an agent package from a tar.gz file
    async fn load_from_file(package_path: &Path) -> Result<Self> {
        let span = spans::load_agent_package(package_path);
        let _guard = span.enter();

        // Create temporary extraction directory
        let epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let extract_dir = std::env::temp_dir().join(format!("baml-agent-{}", epoch_secs));
        std::fs::create_dir_all(&extract_dir)
            .map_err(BamlRtError::Io)?;

        {
            let extract_span = spans::extract_package(&extract_dir);
            let _extract_guard = extract_span.enter();

            // Extract tar.gz
            let tar_gz = std::fs::File::open(package_path)
                .map_err(BamlRtError::Io)?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);

            archive
                .unpack(&extract_dir)
                .map_err(BamlRtError::Io)?;
        }

        // Load manifest
        let manifest_path = extract_dir.join("manifest.json");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(BamlRtError::Io)?;
        let manifest_json: Value = serde_json::from_str(&manifest_content)
            .map_err(BamlRtError::Json)?;

        let manifest = AgentManifest {
            version: manifest_json
                .get("version")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BamlRtError::InvalidArgument(
                    "manifest.json missing 'version' field".to_string()
                ))?
                .to_string(),
            name: manifest_json
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BamlRtError::InvalidArgument(
                    "manifest.json missing 'name' field".to_string()
                ))?
                .to_string(),
            entry_point: manifest_json
                .get("entry_point")
                .and_then(|v| v.as_str())
                .unwrap_or("dist/index.js")
                .to_string(),
        };

        info!(
            name = manifest.name,
            version = manifest.version,
            entry_point = manifest.entry_point,
            "Agent manifest loaded"
        );

        // Validate package structure
        let baml_src = extract_dir.join("baml_src");
        if !baml_src.exists() {
            return Err(BamlRtError::InvalidArgument(
                "Package missing baml_src directory".to_string()
            ));
        }

        // Create runtime manager
        let mut runtime_manager = BamlRuntimeManager::new()?;
        
        // Load BAML schema
        {
            let schema_span = spans::load_baml_schema(&baml_src);
            let _schema_guard = schema_span.enter();
            let baml_src_str = baml_src.to_str()
                .ok_or_else(|| BamlRtError::InvalidArgument(
                    "BAML source path contains invalid UTF-8".to_string()
                ))?;
            runtime_manager.load_schema(baml_src_str)?;
            info!(agent = manifest.name, "BAML schema loaded");
        }

        // Create QuickJS bridge and expose BAML functions to it
        let runtime_manager_arc = Arc::new(Mutex::new(runtime_manager));
        let bridge = {
            let bridge_span = spans::create_js_bridge();
            let _bridge_guard = bridge_span.enter();
            let mut bridge = QuickJSBridge::new(runtime_manager_arc.clone()).await?;
            bridge.register_baml_functions().await?;
            info!(agent = manifest.name, "BAML functions registered with QuickJS");
            Arc::new(Mutex::new(bridge))
        };

        // Load agent's JavaScript code from dist/entry_point
        let entry_point_path = extract_dir.join(&manifest.entry_point);
        if entry_point_path.exists() {
            let eval_span = spans::evaluate_agent_code(&manifest.entry_point);
            let _eval_guard = eval_span.enter();

            let agent_code = std::fs::read_to_string(&entry_point_path)
                .map_err(BamlRtError::Io)?;
            
            info!(entry_point = manifest.entry_point, "Loading agent JavaScript code");

            // Execute the agent's code to initialize it
            // The code should expose functions that can be called later
            // We ignore the result since it's just initialization code
            match bridge.lock().await.evaluate(&agent_code).await {
                Ok(_) => info!("Agent code executed successfully"),
                Err(e) => {
                    // Log warning but don't fail - the code might just not return a value
                    tracing::warn!(
                        error = %e,
                        "Agent code execution returned an error (may be expected)"
                    );
                }
            }

            info!("Agent JavaScript code loaded and initialized");
        } else {
            info!(
                entry_point = manifest.entry_point,
                "Agent entry point not found, skipping JavaScript initialization"
            );
        }

        let agent = A2aAgent::builder()
            .with_runtime_handle(runtime_manager_arc)
            .with_bridge_handle(bridge)
            .with_baml_helpers(false)
            .build()
            .await?;

        Ok(Self {
            name: manifest.name,
            agent,
        })
    }

    /// Get the agent name
    fn name(&self) -> &str {
        &self.name
    }

    /// Execute a function in this agent
    /// 
    /// Calls a JavaScript function exposed by the agent.
    async fn invoke_function(&self, function_name: &str, args: Value) -> Result<Value> {
        // Delegate to QuickJSBridge's JS-only invocation
        let bridge = self.agent.bridge();
        let mut js_bridge = bridge.lock().await;
        js_bridge.invoke_js_function(function_name, args).await
    }

    async fn handle_a2a(&self, request: Value) -> Result<Vec<Value>> {
        self.agent.handle_a2a(request).await
    }

}

/// Agent runner that manages multiple agent packages
struct AgentRunner {
    agents: HashMap<String, AgentPackage>,
}

impl AgentRunner {
    fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Load an agent package
    async fn load_agent(&mut self, package_path: &Path) -> Result<()> {
        let agent = AgentPackage::load_from_file(package_path).await?;
        let name = agent.name().to_string();
        info!(agent = name, "Agent loaded successfully");
        self.agents.insert(name.clone(), agent);
        Ok(())
    }

    /// Execute a function in a specific agent
    async fn invoke(
        &self,
        agent_name: &str,
        function_name: &str,
        args: Value,
    ) -> Result<Value> {
        let span = spans::invoke_function(agent_name, function_name);
        let _guard = span.enter();

        let agent = self.agents.get(agent_name)
            .ok_or_else(|| BamlRtError::InvalidArgument(
                format!("Agent '{}' not found", agent_name)
            ))?;
        
        agent.invoke_function(function_name, args).await
    }

    /// List all loaded agents
    fn list_agents(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    async fn run_a2a_stdio(&self) -> Result<()> {
        use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

        let stdin = io::stdin();
        let mut lines = io::BufReader::new(stdin).lines();
        let mut stdout = io::stdout();

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut request_value: Value = match serde_json::from_str(line) {
                Ok(value) => value,
                Err(err) => {
                    let response = a2a::error_response(
                        None,
                        -32700,
                        "JSON parse error",
                        Some(Value::String(err.to_string())),
                    );
                    let serialized = serde_json::to_string(&response)
                        .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string());
                    stdout.write_all(serialized.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            let request_id = a2a::extract_jsonrpc_id(&request_value);
            let (agent_name, prepared_request) = match self.prepare_a2a_request(&mut request_value) {
                Ok(result) => result,
                Err(err) => {
                    let response = map_a2a_error(request_id, err);
                    let serialized = serde_json::to_string(&response)
                        .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string());
                    stdout.write_all(serialized.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            let agent = match self.agents.get(&agent_name) {
                Some(agent) => agent,
                None => {
                    let response = a2a::error_response(
                        request_id,
                        -32601,
                        "Agent not found",
                        Some(Value::String(agent_name)),
                    );
                    let serialized = serde_json::to_string(&response)
                        .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string());
                    stdout.write_all(serialized.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            let responses = agent
                .handle_a2a(prepared_request)
                .await
                .unwrap_or_else(|err| vec![map_a2a_error(request_id, err)]);
            for response in responses {
                let serialized = serde_json::to_string(&response)
                    .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string());
                stdout.write_all(serialized.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
            }
            stdout.flush().await?;
        }

        Ok(())
    }

    fn prepare_a2a_request(&self, request: &mut Value) -> Result<(String, Value)> {
        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BamlRtError::InvalidArgument("A2A request missing method".to_string()))?
            .to_string();

        if is_a2a_method(&method) {
            let agent_name = a2a::extract_agent_name(request).or_else(|| {
                request
                    .get("params")
                    .and_then(|params| params.get("agent"))
                    .and_then(|agent| agent.as_str())
                    .map(|agent| agent.to_string())
            });
            if let Some(agent_name) = agent_name {
                return Ok((agent_name, request.clone()));
            }
            if self.agents.len() == 1 {
                let agent_name = self.agents.keys().next().cloned().unwrap_or_default();
                return Ok((agent_name, request.clone()));
            }
            return Err(BamlRtError::InvalidArgument(
                "A2A request missing agent (set message metadata agent or params.agent)".to_string(),
            ));
        }

        let obj = request.as_object_mut().ok_or_else(|| {
            BamlRtError::InvalidArgument("A2A request must be a JSON object".to_string())
        })?;
        let (method_base, had_stream_suffix) = strip_stream_suffix(&method);
        let params_value = obj.remove("params").unwrap_or(Value::Null);
        let mut params = match params_value {
            Value::Object(map) => map,
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                map
            }
        };

        let agent_name = if let Some(agent_value) = params.remove("agent") {
            agent_value.as_str().map(|s| s.to_string())
        } else {
            None
        };

        let (agent_name, method_name) = if let Some(agent_name) = agent_name {
            (agent_name, method_base)
        } else if let Some((agent_name, method_name)) = split_agent_method(&method_base, &self.agents) {
            (agent_name, method_name)
        } else if self.agents.len() == 1 {
            let agent_name = self.agents.keys().next().cloned().unwrap_or_default();
            (agent_name, method_base)
        } else {
            return Err(BamlRtError::InvalidArgument(
                "A2A request missing agent (set params.agent or prefix method with agent name)"
                    .to_string(),
            ));
        };

        if had_stream_suffix {
            params.insert("stream".to_string(), Value::Bool(true));
        }

        obj.insert("method".to_string(), Value::String(method_name));
        obj.insert("params".to_string(), Value::Object(params));

        Ok((agent_name, request.clone()))
    }
}

fn strip_stream_suffix(method: &str) -> (String, bool) {
    for suffix in ["/stream", ".stream", ":stream"] {
        if let Some(stripped) = method.strip_suffix(suffix) {
            return (stripped.to_string(), true);
        }
    }
    (method.to_string(), false)
}

fn split_agent_method(method: &str, agents: &HashMap<String, AgentPackage>) -> Option<(String, String)> {
    for sep in ["::", "/", "."] {
        if let Some((prefix, suffix)) = method.split_once(sep)
            && agents.contains_key(prefix)
        {
            return Some((prefix.to_string(), suffix.to_string()));
        }
    }
    None
}

fn is_a2a_method(method: &str) -> bool {
    method.starts_with("message/")
        || method.starts_with("tasks/")
        || method.starts_with("agent/")
}

fn map_a2a_error(id: Option<JSONRPCId>, err: BamlRtError) -> Value {
    match err {
        BamlRtError::InvalidArgument(message) => a2a::error_response(id, -32602, "Invalid params", Some(Value::String(message))),
        BamlRtError::FunctionNotFound(message) => a2a::error_response(id, -32601, "Method not found", Some(Value::String(message))),
        BamlRtError::QuickJs(message) => a2a::error_response(id, -32000, "QuickJS error", Some(Value::String(message))),
        other => a2a::error_response(id, -32603, "Internal error", Some(Value::String(other.to_string()))),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_setup::init_tracing();

    info!("BAML Agent Runner starting");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <agent-package.tar.gz> [agent-package2.tar.gz ...] [--invoke <agent> <function> <json-args>] [--a2a-stdio]", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} agent1.tar.gz agent2.tar.gz", args[0]);
        eprintln!("  {} agent1.tar.gz --invoke agent1 SimpleGreeting '{{\"name\":\"World\"}}'", args[0]);
        eprintln!("  {} agent1.tar.gz --a2a-stdio", args[0]);
        std::process::exit(1);
    }

    let mut runner = AgentRunner::new();
    let mut a2a_stdio = false;

    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--invoke" {
            // Invoke mode
            if i + 3 >= args.len() {
                eprintln!("Error: --invoke requires <agent> <function> <json-args>");
                std::process::exit(1);
            }
            
            let agent_name = &args[i + 1];
            let function_name = &args[i + 2];
            let json_args = &args[i + 3];
            
            let args_value: Value = serde_json::from_str(json_args)
                .context("Invalid JSON arguments")?;
            
            let result = runner.invoke(agent_name, function_name, args_value).await
                .context("Function invocation failed")?;
            
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        } else if args[i] == "--a2a-stdio" {
            a2a_stdio = true;
        } else {
            // Load agent package
            let package_path = Path::new(&args[i]);
            if !package_path.exists() {
                eprintln!("Error: Agent package not found: {}", package_path.display());
                std::process::exit(1);
            }
            
            match runner.load_agent(package_path).await {
                Ok(_) => {
                    info!(package_path = %package_path.display(), "Agent package loaded");
                }
                Err(e) => {
                    error!(error = %e, package = %package_path.display(), "Failed to load agent package");
                    eprintln!("Error: Failed to load agent package {}: {}", package_path.display(), e);
                    std::process::exit(1);
                }
            }
        }
        i += 1;
    }

    // If we get here, just loaded agents without invoking
    let agents = runner.list_agents();
    if agents.is_empty() {
        eprintln!("Error: No agents loaded");
        std::process::exit(1);
    }

    println!("✅ Loaded {} agent(s):", agents.len());
    for agent_name in &agents {
        println!("  - {}", agent_name);
    }

    if a2a_stdio {
        runner.run_a2a_stdio().await?;
        return Ok(());
    }

    info!("Agent Runner completed successfully");
    Ok(())
}
