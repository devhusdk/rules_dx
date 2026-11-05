use crate::{link, Ctx, Plugin};

pub const PLUGIN: Plugin = Plugin {
    name: "typescript",
    extensions: &["ts", "tsx", "mts", "cts"],
    labels: &["typescript/ide:tsdk"],
    outputs: &["lib"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let package = ctx
        .outputs
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "typescript"))
        .min_by_key(|path| path.as_os_str().len())
        .ok_or("the TypeScript package was not built")?;
    link(ctx.dir, "lib", &package.join("lib"))
        .map_err(|error| format!("linking the TypeScript lib: {error}"))
}
