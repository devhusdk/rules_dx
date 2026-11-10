use crate::{Ctx, Plugin, link};
use dx_error::DxError;

pub const PLUGIN: Plugin = Plugin {
    name: "typescript",
    extensions: &["ts", "tsx", "mts", "cts"],
    labels: &["typescript/private/ide:tsdk"],
    outputs: &["lib"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    let package = ctx
        .outputs
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "typescript"))
        .min_by_key(|path| path.as_os_str().len())
        .ok_or_else(|| DxError::failure("the TypeScript package was not built"))?;
    link(ctx.dir, "lib", &package.join("lib"))
        .map_err(|error| DxError::from_io(format!("linking the TypeScript lib: {error}"), &error))
}
