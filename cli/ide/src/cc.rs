use dx_error::DxError;
use std::collections::BTreeMap;

use std::path::PathBuf;

use cc_context::{AqueryGraph, Entry, read_identities};

use crate::{Ctx, Plugin, link};

const AQUERY: &str = r#"mnemonic("CppCompile|ObjcCompile", //...)"#;

pub const PLUGIN: Plugin = Plugin {
    name: "cc",
    extensions: &["c", "cc", "cpp", "cxx", "h", "hh", "hpp", "m", "mm"],
    labels: &[
        "cc/private/context:clangd",
        "cc/private/context:identity",
        "cc/private/context:identity_exec",
    ],
    outputs: &["compile_commands.json", "clangd"],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), DxError> {
    let clangd = ctx
        .outputs
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "clangd" || name == "clangd.exe")
        })
        .ok_or_else(|| DxError::failure("clangd was not built"))?;
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
        return Err(DxError::failure("the compiler identity was not built"));
    }
    let identities =
        read_identities(&identity_paths).map_err(|e| DxError::failure(e.to_string()))?;
    let execroot = ctx.runner.execution_root()?;
    let graph = AqueryGraph::parse(&ctx.runner.aquery(AQUERY)?)
        .map_err(|e| DxError::failure(e.to_string()))?;
    let commands = graph
        .compile_commands(&execroot, &identities)
        .map_err(|e| DxError::failure(e.to_string()))?;
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
    let mut text =
        serde_json::to_string_pretty(&entries).map_err(|e| DxError::infra(e.to_string()))?;
    text.push('\n');
    std::fs::create_dir_all(ctx.dir)
        .map_err(|e| DxError::infra(format!("creating {}: {e}", ctx.dir.display())))?;
    dx_atomic_fs::write_atomic(&ctx.dir.join("compile_commands.json"), text.as_bytes())
        .map_err(|e| DxError::infra(format!("writing compile_commands.json: {e}")))?;
    link(ctx.dir, "clangd", clangd).map_err(|e| DxError::infra(format!("linking clangd: {e}")))
}
