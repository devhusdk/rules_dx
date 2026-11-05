use std::collections::BTreeMap;

use std::path::PathBuf;

use cc_context::{read_identities, AqueryGraph, Entry};

use crate::{link, Ctx, Plugin};

const AQUERY: &str = r#"mnemonic("CppCompile|ObjcCompile", //...)"#;

pub const PLUGIN: Plugin = Plugin {
    name: "cc",
    extensions: &["c", "cc", "cpp", "cxx", "h", "hh", "hpp", "m", "mm"],
    labels: &[
        "cc/context:clangd",
        "cc/context:identity",
        "cc/context:identity_exec",
    ],
    outputs: &["compile_commands.json", "clangd"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let clangd = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "clangd" || name == "clangd.exe")
        })
        .ok_or("clangd was not built")?;
    let identity_paths: Vec<PathBuf> = ctx
        .outputs
        .iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".cc_identity.json"))
        })
        .cloned()
        .collect();
    if identity_paths.is_empty() {
        return Err("the compiler identity was not built".to_owned());
    }
    let identities = read_identities(&identity_paths).map_err(|e| e.to_string())?;
    let execroot = ctx.runner.execution_root()?;
    let graph = AqueryGraph::parse(&ctx.runner.aquery(AQUERY)?).map_err(|e| e.to_string())?;
    let commands = graph
        .compile_commands(&execroot, &identities)
        .map_err(|e| e.to_string())?;
    let mut by_file: BTreeMap<String, Entry> = BTreeMap::new();
    for command in commands {
        let file = execroot
            .join(&command.source)
            .to_string_lossy()
            .into_owned();
        by_file.entry(file.clone()).or_insert(Entry {
            directory: execroot.to_string_lossy().into_owned(),
            file,
            arguments: command.arguments,
        });
    }
    let entries: Vec<Entry> = by_file.into_values().collect();
    let mut text = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::create_dir_all(ctx.dir).map_err(|e| format!("creating {}: {e}", ctx.dir.display()))?;
    dx_atomic_fs::write_atomic(&ctx.dir.join("compile_commands.json"), text.as_bytes())
        .map_err(|e| format!("writing compile_commands.json: {e}"))?;
    link(ctx.dir, "clangd", clangd).map_err(|e| format!("linking clangd: {e}"))
}
