use crate::{Ctx, Plugin, link};
use dx_error::DxError;

pub const PLUGIN: Plugin = Plugin {
    name: "dotnet",
    extensions: &["cs", "fs", "fsi", "fsx"],
    labels: &[
        "csharp/private/ide:dotnet_tool",
        "fsharp/private/ide:dotnet_tool",
    ],
    outputs: &["root"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    let root = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "dotnet" || name == "dotnet.exe")
        })
        .and_then(|dotnet| dotnet.parent())
        .ok_or_else(|| DxError::failure("the .NET SDK was not built"))?;
    link(ctx.dir, "root", root)
        .map_err(|error| DxError::from_io(format!("linking the .NET SDK root: {error}"), &error))
}
