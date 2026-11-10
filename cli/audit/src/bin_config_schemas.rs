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
        .map_err(|_| "run with `bazel run //cli/audit:config_schemas`")?;
    let dir = PathBuf::from(root).join("cli/audit/schema");
    std::fs::create_dir_all(&dir)?;
    let files = [
        (
            "security.schema.json",
            dx_audit::config_schema::security_schema(),
        ),
        (
            "license.schema.json",
            dx_audit::config_schema::licenses_schema(),
        ),
    ];
    for (name, text) in files {
        std::fs::write(dir.join(name), text)?;
    }
    Ok(())
}
