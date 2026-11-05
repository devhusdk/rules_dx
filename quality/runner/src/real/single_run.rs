use quality_adapter::parsers;

use super::check::{CheckCall, ToolDef};
use super::*;

fn paths<'a>(call: &'a CheckCall<'a>) -> (Vec<&'a Path>, Vec<String>) {
    let refs = call
        .staged
        .pairs
        .iter()
        .map(|(_, absolute)| absolute.as_path())
        .collect();
    let names = call
        .staged
        .pairs
        .iter()
        .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
        .collect();
    (refs, names)
}

fn run(
    backend: &RealBackend,
    call: &CheckCall,
    invocation: &Invocation,
) -> Result<quality_adapter::exec::ChildOutput, RunnerError> {
    backend.run(call.tool_id, call.tool, invocation, &call.staged.scratch)
}

/// Tools that check every file in one process and parse its output.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "hlint",
        run: |backend, call| {
            let Some(config) = call.config else {
                return Err(execution(
                    call.tool_id,
                    "hlint needs a config path".to_owned(),
                ));
            };
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::hlint_check(&call.tool.binary, &refs, config),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_hlint(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "xmllint",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::xmllint_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_xmllint(&out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "jsonnet_lint",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::jsonnet_lint_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_jsonnet_lint(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "jsonnetfmt",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::jsonnetfmt_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_jsonnetfmt(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "dscanner",
        run: |backend, call| {
            // dscanner silently skips a file it cannot read, so read each one first.
            for (workspace, absolute) in &call.staged.pairs {
                std::fs::read(absolute)
                    .map_err(|err| execution(call.tool_id, format!("read {workspace}: {err}")))?;
            }
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::dscanner_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_dscanner(&out.stdout, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "perlcritic",
        run: |backend, call| {
            // perlcritic exits 3 on a file it cannot read, so it is never a clean run.
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::perlcritic_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_perlcritic(&out.stdout, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "typstyle",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::typstyle_check(&call.tool.binary, &refs),
            )?;
            let cwd_rel = RealBackend::cwd_rel(call.tool_id, call.tool.config_rel.as_deref());
            let cwd = call.staged.scratch.root().join(&cwd_rel);
            parsed(
                call.tool_id,
                parsers::parse_typstyle(&out.stdout, &out.stderr, out.code, &strs, &cwd),
            )
        },
    },
];
