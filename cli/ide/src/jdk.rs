use crate::{link, Ctx, Plugin};

pub const PLUGIN: Plugin = Plugin {
    name: "jdk",
    extensions: &["java", "kt", "kts"],
    labels: &["java/ide:java_tool"],
    outputs: &["home"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let home = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "java" || name == "java.exe")
        })
        .and_then(|java| java.parent()?.parent())
        .ok_or("the JDK was not built")?;
    link(ctx.dir, "home", home).map_err(|error| format!("linking the JDK home: {error}"))
}
