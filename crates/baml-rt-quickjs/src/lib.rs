//! BAML runtime with QuickJS integration.

pub mod baml;
pub mod baml_collector;
pub mod baml_execution;
pub mod baml_pre_execution;
pub mod context;
pub mod js_value_converter;
pub mod quickjs_bridge;
pub mod runtime;
pub mod traits;

pub use baml::BamlRuntimeManager;
pub use quickjs_bridge::QuickJSBridge;
pub use runtime::{QuickJSConfig, Runtime, RuntimeBuilder, RuntimeConfig};
pub use context::{BamlContext, ContextMetadata};
pub use traits::{BamlFunctionExecutor, BamlGateway, JsRuntimeHost, SchemaLoader, ToolRegistryTrait};
