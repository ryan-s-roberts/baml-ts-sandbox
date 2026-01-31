//! Agent builder module
//!
//! Provides production-grade abstractions for building, linting, and packaging
//! BAML agent applications.

pub mod types;
pub mod traits;
pub mod filesystem;
pub mod linter;
pub mod compiler;
pub mod packager;
pub mod service;

pub use types::{AgentDir, PackagePath, FunctionName, BuildDir};
pub use traits::{
    Linter, TypeScriptCompiler, TypeGenerator, FileSystem, Packager
};
pub use filesystem::StdFileSystem;
pub use linter::OxcLinter;
pub use compiler::{OxcTypeScriptCompiler, RuntimeTypeGenerator};
pub use packager::StdPackager;
pub use service::BuilderService;
