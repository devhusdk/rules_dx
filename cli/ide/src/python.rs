use crate::{Ctx, Plugin, link};

pub const PLUGIN: Plugin = Plugin {
    name: "python",
    extensions: &["py"],
    labels: &["python/ide:python_tool"],
    outputs: &["python"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let interpreter = ctx
        .outputs
        .iter()
        .find(|path| path.file_name().is_some_and(|name| name != "typescript"))
        .ok_or("the Python interpreter was not built")?;
    link(ctx.dir, "python", interpreter)
        .map_err(|error| format!("linking the Python interpreter: {error}"))
}
