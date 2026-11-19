#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! The `api_spec` code generation helpers `dx api_spec_preflight` and `dx oapi_codegen` run.
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

pub mod oapi_codegen;
pub mod preflight;
