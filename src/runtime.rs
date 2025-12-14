//! Runtime builder and configuration
//!
//! Provides a builder pattern for constructing and configuring the BAML runtime environment.

use crate::baml::BamlRuntimeManager;
use crate::error::{BamlRtError, Result};
use crate::quickjs_bridge::QuickJSBridge;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Configuration for QuickJS runtime options
/// 
/// These options map directly to the available options in `quickjs_runtime::builder::QuickJsRuntimeBuilder`.
#[derive(Debug, Clone)]
pub struct QuickJSConfig {
    /// Maximum memory limit in bytes (None = no limit)
    pub memory_limit: Option<u64>,
    
    /// Maximum stack size in bytes (None = default)
    pub max_stack_size: Option<u64>,
    
    /// Number of allocations before garbage collection runs (None = default)
    pub gc_threshold: Option<u64>,
    
    /// Garbage collection interval - triggers a full GC every set interval (None = disabled)
    pub gc_interval: Option<Duration>,
}

impl Default for QuickJSConfig {
    fn default() -> Self {
        Self {
            memory_limit: None,
            max_stack_size: None,
            gc_threshold: None,
            gc_interval: None,
        }
    }
}

impl QuickJSConfig {
    /// Create a new QuickJS configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set memory limit in bytes
    pub fn with_memory_limit(mut self, limit: Option<u64>) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Set maximum stack size in bytes
    pub fn with_max_stack_size(mut self, size: Option<u64>) -> Self {
        self.max_stack_size = size;
        self
    }

    /// Set garbage collection threshold (number of allocations before GC runs)
    pub fn with_gc_threshold(mut self, threshold: Option<u64>) -> Self {
        self.gc_threshold = threshold;
        self
    }

    /// Set garbage collection interval
    /// 
    /// This will start a timer thread which triggers a full GC every set interval.
    pub fn with_gc_interval(mut self, interval: Option<Duration>) -> Self {
        self.gc_interval = interval;
        self
    }
}

/// Configuration for the BAML runtime environment
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the BAML schema directory
    pub schema_path: Option<PathBuf>,
    
    /// Whether to enable QuickJS bridge
    pub enable_quickjs: bool,
    
    /// QuickJS-specific configuration (only used if enable_quickjs is true)
    pub quickjs_config: QuickJSConfig,
    
    /// Additional environment variables to pass to BAML runtime
    pub env_vars: Vec<(String, String)>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            schema_path: None,
            enable_quickjs: false,
            quickjs_config: QuickJSConfig::default(),
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

    /// Configure QuickJS runtime options
    /// 
    /// This allows fine-grained control over the QuickJS runtime behavior,
    /// including memory limits, stack size, and module loading.
    pub fn with_quickjs_config(mut self, config: QuickJSConfig) -> Self {
        self.quickjs_config = config;
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

    /// Configure QuickJS runtime options
    /// 
    /// This allows fine-grained control over the QuickJS runtime behavior,
    /// including memory limits, stack size, and garbage collection settings.
    /// 
    /// # Example
    /// ```rust,no_run
    /// use baml_rt::{RuntimeBuilder, QuickJSConfig};
    /// use std::time::Duration;
    /// 
    /// let runtime = RuntimeBuilder::new()
    ///     .with_quickjs(true)
    ///     .with_quickjs_config(
    ///         QuickJSConfig::new()
    ///             .with_memory_limit(Some(64 * 1024 * 1024)) // 64MB limit
    ///             .with_max_stack_size(Some(1024 * 1024)) // 1MB stack
    ///             .with_gc_interval(Some(Duration::from_secs(30))) // GC every 30 seconds
    ///     )
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_quickjs_config(mut self, config: QuickJSConfig) -> Self {
        self.config.quickjs_config = config;
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
            let mut bridge = QuickJSBridge::new_with_config(
                baml_manager.clone(),
                self.config.quickjs_config.clone(),
            )?;
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

