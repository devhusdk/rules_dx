#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|_| "run with `bazel run //cli/dx_config:update_schema`")?;
    let path = PathBuf::from(root).join("cli/dx_config/schema/dx.schema.json");
    std::fs::write(path, dx_config::schema::dx_schema())?;
    Ok(())
}
