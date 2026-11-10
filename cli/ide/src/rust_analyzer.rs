use dx_error::DxError;
use std::path::{Path, PathBuf};

use crate::link::make_executable;
use crate::{Ctx, Plugin};

pub const PLUGIN: Plugin = Plugin {
    name: "rust_analyzer",
    extensions: &["rs"],
    labels: &["rust/ide:flycheck", "rust/ide:discover"],
    outputs: &[
        "flycheck.exe",
        "discover_bazel_rust_project.exe",
        "user_config.json",
    ],
    write,
};

const USER_CONFIG: &str = "{\"clippy\": true}\n";

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    std::fs::create_dir_all(ctx.dir)
        .map_err(|e| DxError::infra(format!("creating {}: {e}", ctx.dir.display())))?;
    copy(ctx, "flycheck")?;
    copy(ctx, "discover_bazel_rust_project")?;
    dx_atomic_fs::write_atomic(&ctx.dir.join("user_config.json"), USER_CONFIG.as_bytes())
        .map_err(|e| DxError::infra(format!("writing user_config.json: {e}")))
}

/// Copies the built launcher to `<name>.exe`. A copy, because rules_rust finds its sidecar files
/// next to the resolved executable.
fn copy(ctx: &Ctx<'_>, name: &str) -> Result<(), DxError> {
    let exe = format!("{name}.exe");
    let source: &PathBuf = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|file| file == name || file == exe.as_str())
        })
        .ok_or_else(|| DxError::failure(format!("{name} was not built")))?;
    let bytes = std::fs::read(source)
        .map_err(|e| DxError::infra(format!("reading {}: {e}", source.display())))?;
    let target: &Path = &ctx.dir.join(&exe);
    dx_atomic_fs::write_atomic(target, &bytes)
        .map_err(|e| DxError::infra(format!("writing {exe}: {e}")))?;
    make_executable(target).map_err(|e| DxError::infra(format!("making {exe} executable: {e}")))
}
