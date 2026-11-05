use crate::link::make_executable;
use crate::{Ctx, Plugin};

pub const PLUGIN: Plugin = Plugin {
    name: "go",
    extensions: &["go"],
    labels: &["go/ide:gopackagesdriver"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let driver = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "gopackagesdriver" || name == "gopackagesdriver.exe")
        })
        .ok_or("the Go packages driver was not built")?;
    let script = format!(
        "#!/bin/sh\nBUILD_WORKSPACE_DIRECTORY={} exec {} \"$@\"\n",
        quote(&ctx.workspace.to_string_lossy()),
        quote(&driver.to_string_lossy()),
    );
    std::fs::create_dir_all(ctx.dir).map_err(|e| format!("creating {}: {e}", ctx.dir.display()))?;
    let path = ctx.dir.join("gopackagesdriver");
    dx_atomic_fs::write_atomic(&path, script.as_bytes())
        .map_err(|e| format!("writing gopackagesdriver: {e}"))?;
    make_executable(&path).map_err(|e| format!("making gopackagesdriver executable: {e}"))
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}
