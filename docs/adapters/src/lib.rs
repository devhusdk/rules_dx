#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod adapters;

mod common;
mod types;

pub use adapters::cpp::normalize_cpp;
pub use adapters::csharp::normalize_csharp;
pub use adapters::fsharp::normalize_fsharp;
pub use adapters::go::normalize_go;
pub use adapters::java::normalize_java;
pub use adapters::kotlin::normalize_kotlin;
pub use adapters::prose::confirm_prose_only;
pub use adapters::python::normalize_python;
pub use adapters::rust::normalize_rust;
pub use adapters::scala::normalize_scala;
pub use adapters::svelte::normalize_svelte;
pub use adapters::typescript::normalize_typescript;
pub use adapters::vue::normalize_vue;
pub use common::encode_ir;
pub use types::AdapterError;

pub const RUST_RUSTDOC_PIN: &str = "nightly-2026-09-01";
pub const RUST_FORMAT_VERSION: u32 = 30;
pub const PYTHON_GRIFFE_PIN: &str = "2.2.0";
pub const TYPESCRIPT_TYPEDOC_PIN: &str = "0.28.20";
pub const JAVA_JDK_PIN: &str = "25";
pub const KOTLIN_PIN: &str = "2.2.20";
pub const KOTLIN_DOKKA_PIN: &str = "2.2.0";
pub const GO_TOOLCHAIN_PIN: &str = "1.26.6";
pub const GO_XTOOLS_PIN: &str = "v0.36.0";
pub const CPP_DOXYGEN_PIN: &str = "1.18.0";
pub const CSHARP_DOTNET_PIN: &str = "10.0.201";
pub const FSHARP_DOTNET_PIN: &str = "10.0.201";
pub const FSHARP_SERVICE_PIN: &str = "43.9.200";
pub const VUE_DOCGEN_PIN: &str = "4.79.2";
pub const SVELTE_SVELD_PIN: &str = "0.37.3";
pub const SCALA_PIN: &str = "3.3.6";

pub const ADAPTER_SCOPES: &[&str] = &[
    "rust",
    "python",
    "typescript",
    "java",
    "kotlin",
    "go",
    "cpp",
    "csharp",
    "fsharp",
    "vue",
    "svelte",
    "scala",
    "astromdx",
];

#[cfg(test)]
mod tests;
