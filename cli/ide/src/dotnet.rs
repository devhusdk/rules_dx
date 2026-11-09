use crate::{Ctx, Plugin, link};

pub const PLUGIN: Plugin = Plugin {
    name: "dotnet",
    extensions: &["cs", "fs", "fsi", "fsx"],
    labels: &[
        "families/dotnet/csharp/ide:dotnet_tool",
        "families/dotnet/fsharp/ide:dotnet_tool",
    ],
    outputs: &["root"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let root = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "dotnet" || name == "dotnet.exe")
        })
        .and_then(|dotnet| dotnet.parent())
        .ok_or("the .NET SDK was not built")?;
    link(ctx.dir, "root", root).map_err(|error| format!("linking the .NET SDK root: {error}"))
}
