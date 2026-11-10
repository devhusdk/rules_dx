use crate::{Ctx, Plugin, link};
use dx_error::DxError;

pub const PLUGIN: Plugin = Plugin {
    name: "jdk",
    extensions: &["java", "kt", "kts", "scala", "sc"],
    labels: &["java/ide:java_tool"],
    outputs: &["home"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    let home = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "java" || name == "java.exe")
        })
        .and_then(|java| java.parent()?.parent())
        .ok_or_else(|| DxError::failure("the JDK was not built"))?;
    link(ctx.dir, "home", home)
        .map_err(|error| DxError::infra(format!("linking the JDK home: {error}")))
}
