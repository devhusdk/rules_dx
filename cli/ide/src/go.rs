use crate::link::make_executable;
use crate::{Ctx, Plugin};
use dx_error::DxError;

const WRAPPER: &str = if cfg!(windows) {
    "gopackagesdriver.cmd"
} else {
    "gopackagesdriver"
};

pub const PLUGIN: Plugin = Plugin {
    name: "go",
    extensions: &["go"],
    labels: &["go/ide:gopackagesdriver"],
    outputs: &[WRAPPER],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    let driver = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "gopackagesdriver" || name == "gopackagesdriver.exe")
        })
        .ok_or_else(|| DxError::failure("the Go packages driver was not built"))?;
    let script = script(&ctx.workspace.to_string_lossy(), &driver.to_string_lossy());
    std::fs::create_dir_all(ctx.dir)
        .map_err(|e| DxError::infra(format!("creating {}: {e}", ctx.dir.display())))?;
    let path = ctx.dir.join(WRAPPER);
    dx_atomic_fs::write_atomic(&path, script.as_bytes())
        .map_err(|e| DxError::infra(format!("writing {WRAPPER}: {e}")))?;
    make_executable(&path).map_err(|e| DxError::infra(format!("making {WRAPPER} executable: {e}")))
}

#[cfg(not(windows))]
fn script(workspace: &str, driver: &str) -> String {
    format!(
        "#!/bin/sh\nBUILD_WORKSPACE_DIRECTORY={} exec {} \"$@\"\n",
        quote(workspace),
        quote(driver),
    )
}

#[cfg(windows)]
fn script(workspace: &str, driver: &str) -> String {
    format!(
        "@set \"BUILD_WORKSPACE_DIRECTORY={}\"\r\n@\"{}\" %*\r\n",
        cmd_quote(workspace),
        cmd_quote(driver),
    )
}

#[cfg(not(windows))]
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn cmd_quote(text: &str) -> String {
    text.replace('%', "%%")
}
