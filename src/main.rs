//! BAML Runtime - Main entry point

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("BAML Runtime starting");

    // Initialize BAML runtime manager
    let baml_manager = Arc::new(Mutex::new(BamlRuntimeManager::new()?));

    // Initialize QuickJS bridge
    let _bridge = QuickJSBridge::new(baml_manager.clone())?;

    tracing::info!("BAML Runtime initialized");

    // TODO: Add actual runtime loop or API server here

    Ok(())
}
