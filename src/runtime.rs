//! Runtime builder and configuration
//!
//! Provides a builder pattern for constructing and configuring the BAML runtime environment.

use crate::baml::BamlRuntimeManager;
use crate::error::{BamlRtError, Result};
use crate::quickjs_bridge::QuickJSBridge;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the BAML runtime environment
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the BAML schema directory
    pub schema_path: Option<PathBuf>,
    
    /// Whether to enable QuickJS bridge
    pub enable_quickjs: bool,
    
    /// Additional environment variables to pass to BAML runtime
    pub env_vars: Vec<(String, String)>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            schema_path: None,
            enable_quickjs: false,
            env_vars: Vec::new(),
        }
    }
}

impl RuntimeConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the BAML schema path
    pub fn with_schema_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.schema_path = Some(path.into());
        self
    }

    /// Enable QuickJS bridge
    pub fn with_quickjs(mut self, enable: bool) -> Self {
        self.enable_quickjs = enable;
        self
    }

    /// Add an environment variable
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }
}

/// Built runtime environment
pub struct Runtime {
    /// The BAML runtime manager
    pub baml_manager: Arc<Mutex<BamlRuntimeManager>>,
    
    /// The QuickJS bridge (if enabled)
    pub quickjs_bridge: Option<Arc<Mutex<QuickJSBridge>>>,
    
    /// Runtime configuration
    pub config: RuntimeConfig,
}

impl Runtime {
    /// Get the BAML runtime manager
    pub fn baml_manager(&self) -> Arc<Mutex<BamlRuntimeManager>> {
        self.baml_manager.clone()
    }

    /// Get the QuickJS bridge (if enabled)
    pub fn quickjs_bridge(&self) -> Option<Arc<Mutex<QuickJSBridge>>> {
        self.quickjs_bridge.clone()
    }

    /// Get mutable access to QuickJS bridge
    pub fn quickjs_bridge_mut(&mut self) -> Option<&mut QuickJSBridge> {
        // This requires interior mutability or restructuring
        // For now, return None if we want to avoid unsafe
        None
    }
}

/// Builder for constructing a runtime environment
pub struct RuntimeBuilder {
    config: RuntimeConfig,
}

impl RuntimeBuilder {
    /// Create a new runtime builder with default configuration
    pub fn new() -> Self {
        Self {
            config: RuntimeConfig::default(),
        }
    }

    /// Set the BAML schema path
    pub fn with_schema_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.schema_path = Some(path.into());
        self
    }

    /// Enable QuickJS bridge
    pub fn with_quickjs(mut self, enable: bool) -> Self {
        self.config.enable_quickjs = enable;
        self
    }

    /// Add an environment variable
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.env_vars.push((key.into(), value.into()));
        self
    }

    /// Build the runtime environment
    pub async fn build(self) -> Result<Runtime> {
        tracing::info!("Building runtime environment");

        // Create BAML runtime manager
        let mut baml_manager = BamlRuntimeManager::new()?;

        // Load schema if path is provided
        if let Some(schema_path) = &self.config.schema_path {
            let schema_path_str = schema_path
                .to_str()
                .ok_or_else(|| BamlRtError::InvalidArgument(
                    format!("Schema path contains invalid UTF-8: {:?}", schema_path)
                ))?;
            baml_manager.load_schema(schema_path_str)?;
        }

        let baml_manager = Arc::new(Mutex::new(baml_manager));

        // Create QuickJS bridge if enabled
        let quickjs_bridge = if self.config.enable_quickjs {
            let mut bridge = QuickJSBridge::new(baml_manager.clone())?;
            bridge.register_baml_functions().await?;
            Some(Arc::new(Mutex::new(bridge)))
        } else {
            None
        };

        Ok(Runtime {
            baml_manager,
            quickjs_bridge,
            config: self.config,
        })
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

