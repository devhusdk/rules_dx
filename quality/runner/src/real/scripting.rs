use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::check::{in_file, real_launcher, CheckCall, ToolDef};
use super::single_run::paths;
use super::*;

/// The bundled standard phpcs runs when the consumer binds no config.
const PHPCS_STANDARD: &str = "PSR12";

/// The rule level PHPStan runs at when the consumer binds no config.
const PHPSTAN_LEVEL: u8 = 5;

/// BEAM, PHP, Ruby, Lua and Haskell tools with their own launch and parse.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "dialyzer",
        run: dialyzer,
    },
    ToolDef {
        id: "xref",
        run: xref,
    },
    ToolDef {
        id: "phpcs",
        run: phpcs,
    },
    ToolDef {
        id: "jet",
        run: jet,
    },
    ToolDef {
        id: "pkl_lsp",
        run: pkl_lsp,
    },
    ToolDef {
        id: "julialint",
        run: julialint,
    },
    ToolDef {
        id: "luals",
        run: luals,
    },
    ToolDef {
        id: "phpstan",
        run: phpstan,
    },
    ToolDef {
        id: "sorbet",
        run: sorbet,
    },
    ToolDef {
        id: "perltidy",
        run: perltidy,
    },
    ToolDef {
        id: "elvis",
        run: elvis,
    },
    ToolDef {
        id: "erlfmt",
        run: erlfmt,
    },
    ToolDef {
        id: "credo",
        run: credo,
    },
    ToolDef {
        id: "mix_format",
        run: mix_format,
    },
    ToolDef {
        id: "selene",
        run: selene,
    },
    ToolDef {
        id: "stylua",
        run: stylua,
    },
    ToolDef {
        id: "ormolu",
        run: ormolu,
    },
    ToolDef {
        id: "standardrb",
        run: standardrb,
    },
    ToolDef {
        id: "rubocop",
        run: lint_family,
    },
    ToolDef {
        id: "yamllint",
        run: lint_family,
    },
    ToolDef {
        id: "shellcheck",
        run: lint_family,
    },
    ToolDef {
        id: "hadolint",
        run: lint_family,
    },
    ToolDef {
        id: "checkmake",
        run: lint_family,
    },
];

fn dialyzer(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let staged = |prefix: &str| -> Vec<PathBuf> {
        let mut rels: Vec<&String> = tool
            .tool_files
            .iter()
            .map(|(rel, _)| rel)
            .filter(|rel| rel.starts_with(prefix))
            .collect();
        rels.sort();
        rels.into_iter()
            .map(|rel| scratch.root().join(rel))
            .collect()
    };
    let plts = staged("dx-dialyzer/plt/");
    let beams: Vec<PathBuf> = staged("dx-dialyzer/owned/")
        .into_iter()
        .chain(staged("dx-dialyzer/aux/"))
        .filter(|path| path.extension().is_some_and(|ext| ext == "beam"))
        .collect();
    // A language's own compiler beams (Elixir's) go on the code path, so dialyzer can
    // read the debug info of that language's beams.
    let code_dir = scratch.root().join("dx-dialyzer/pa");
    let code_dirs: Vec<&Path> = if staged("dx-dialyzer/pa/").is_empty() {
        Vec::new()
    } else {
        vec![code_dir.as_path()]
    };
    let owned_beams = staged("dx-dialyzer/owned/")
        .iter()
        .any(|path| path.extension().is_some_and(|ext| ext == "beam"));
    if !owned_beams {
        // An app with no sources has an empty ebin directory; there is nothing to analyse.
        return Ok(Vec::new());
    }
    let plt_refs: Vec<&Path> = plts.iter().map(PathBuf::as_path).collect();
    let beam_refs: Vec<&Path> = beams.iter().map(PathBuf::as_path).collect();
    let name_refs: Vec<&str> = pairs.iter().map(|(rel, _)| rel.as_str()).collect();
    let invocation = commands::dialyzer_check(&tool.binary, &code_dirs, &plt_refs, &beam_refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let mut findings = parsed(
        tool_id,
        parsers::parse_dialyzer(&out.stdout, out.code, &name_refs),
    )?;
    // The parser names owned files by workspace path; later stages key them by the
    // staged absolute path.
    for found in &mut findings {
        if let Some((_, absolute)) = pairs.iter().find(|(rel, _)| *rel == found.file) {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn xref(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let staged = |prefix: &str| -> Vec<PathBuf> {
        let mut rels: Vec<&String> = tool
            .tool_files
            .iter()
            .map(|(rel, _)| rel)
            .filter(|rel| rel.starts_with(prefix) && rel.ends_with(".beam"))
            .collect();
        rels.sort();
        rels.into_iter()
            .map(|rel| scratch.root().join(rel))
            .collect()
    };
    let owned = staged("dx-xref/owned/");
    if owned.is_empty() {
        return Ok(Vec::new());
    }
    let deps = staged("dx-xref/aux/");
    let code_dir = scratch.root().join("dx-xref/pa");
    let code_dirs: Vec<&Path> = if staged("dx-xref/pa/").is_empty() {
        Vec::new()
    } else {
        vec![code_dir.as_path()]
    };
    let script = scratch.root().join("dx-xref/dx_xref.escript");
    let dep_refs: Vec<&Path> = deps.iter().map(PathBuf::as_path).collect();
    let owned_refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let name_refs: Vec<&str> = pairs.iter().map(|(rel, _)| rel.as_str()).collect();
    let invocation =
        commands::xref_check(&tool.binary, &script, &code_dirs, &dep_refs, &owned_refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let mut findings = parsed(
        tool_id,
        parsers::parse_xref(&out.stdout, out.code, &name_refs),
    )?;
    for found in &mut findings {
        if let Some((_, absolute)) = pairs.iter().find(|(rel, _)| *rel == found.file) {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn phpcs(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let config = call.config;
    // phpcs reports the paths it was given, resolved; pass and match canonical ones.
    let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
    for (workspace, absolute) in pairs {
        let real = std::fs::canonicalize(absolute)
            .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
        owned.push((real, absolute));
    }
    // Always name a standard: without one phpcs would pick up a phpcs.xml from the
    // working directory.
    let standard: std::ffi::OsString = match config {
        Some(path) => std::fs::canonicalize(path)
            .map_err(|err| execution(tool_id, format!("resolve phpcs config: {err}")))?
            .into_os_string(),
        None => PHPCS_STANDARD.into(),
    };
    let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
    let invocation = commands::phpcs_check(&tool.binary, &standard, &path_refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let real_names: Vec<String> = owned
        .iter()
        .map(|(real, _)| real.to_string_lossy().into_owned())
        .collect();
    let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
    let mut findings = parsed(
        tool_id,
        parsers::parse_phpcs(&out.stdout, out.code, &real_strs),
    )?;
    for found in &mut findings {
        if let Some((_, absolute)) = owned
            .iter()
            .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
        {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn jet(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let root = std::fs::canonicalize(scratch.root())
        .map_err(|err| execution(tool_id, format!("canonicalize scratch root: {err}")))?;
    let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
    for (workspace, absolute) in pairs {
        let real = std::fs::canonicalize(absolute)
            .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
        owned.push((real, absolute));
    }
    let work = root.join(".dx-jet");
    std::fs::create_dir_all(&work)
        .map_err(|err| execution(tool_id, format!("stage jet work dir: {err}")))?;
    let report = work.join("report.json");
    let invocation = commands::jet_check(
        &tool.binary,
        &root.join("dx-jet/config.toml"),
        &root,
        &report,
    );
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let body = std::fs::read(&report).map_err(|err| {
        execution(
            tool_id,
            format!(
                "no report (exit {:?}): {err}: {}",
                out.code,
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ),
        )
    })?;
    let real_names: Vec<String> = owned
        .iter()
        .map(|(real, _)| real.to_string_lossy().into_owned())
        .collect();
    let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
    let mut findings = parsed(tool_id, parsers::parse_jet(&body, &real_strs))?;
    for found in &mut findings {
        if let Some((_, absolute)) = owned
            .iter()
            .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
        {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn pkl_lsp(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let companion = |name: &str| {
        tool.path_tools
            .iter()
            .find(|(helper, _)| helper == name)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| {
                execution(
                    tool_id,
                    format!("pkl_lsp needs the {name} companion; the aspect did not supply it"),
                )
            })
    };
    let java = companion("java")?;
    let java = real_launcher(&java);
    let jar = companion("pkl-lsp.jar")?;
    let home = scratch.root().join(PKL_LSP_HOME_REL);
    std::fs::create_dir_all(&home)
        .map_err(|err| execution(tool_id, format!("stage pkl-lsp home: {err}")))?;
    let invocation = commands::pkl_lsp_check(&tool.binary, &java, &jar, &home, &refs);
    // The companions are named by absolute path, so the child needs no PATH shims.
    let bare = RealTool {
        binary: tool.binary.clone(),
        extra_env: tool.extra_env.clone(),
        config_rel: None,
        edition: None,
        dialect: None,
        tool_files: Vec::new(),
        upstream_diagnostics: Vec::new(),
        search_paths: Vec::new(),
        path_tools: Vec::new(),
    };
    let out = backend.run(tool_id, &bare, &invocation, scratch)?;
    parsed(
        tool_id,
        parsers::parse_pkl_lsp(&out.stdout, out.code, &strs),
    )
    .map_err(|err| with_stderr(err, &out.stderr))
}

fn julialint(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let work = scratch.root().join(".dx-julialint");
    std::fs::create_dir_all(&work)
        .map_err(|err| execution(tool_id, format!("stage julialint work dir: {err}")))?;
    let report = work.join("report.json");
    let invocation = commands::julialint_check(&tool.binary, &work, &report, &refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let body = std::fs::read(&report).map_err(|err| {
        execution(
            tool_id,
            format!(
                "no report (exit {:?}): {err}: {}",
                out.code,
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ),
        )
    })?;
    parsed(tool_id, parsers::parse_julialint(&body, out.code, &strs))
}

fn luals(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let config = call.config;
    // LuaLS reports file URIs of the real paths, so analyse and match canonical ones.
    let root = std::fs::canonicalize(scratch.root())
        .map_err(|err| execution(tool_id, format!("canonicalize scratch root: {err}")))?;
    let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
    for (workspace, absolute) in pairs {
        let real = std::fs::canonicalize(absolute)
            .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
        owned.push((real, absolute));
    }
    let work = root.join(".dx-luals");
    std::fs::create_dir_all(&work)
        .map_err(|err| execution(tool_id, format!("stage luals work dir: {err}")))?;
    let library = tool
        .tool_files
        .iter()
        .any(|(rel, _)| rel.starts_with("dx-luals/mod/"))
        .then(|| root.join("dx-luals/mod"));
    // A consumer config is a flat .luarc.json object; .jsonc is not read.
    let consumer: Option<serde_json::Value> = match config {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|err| execution(tool_id, format!("read luals config: {err}")))?;
            Some(serde_json::from_str(&text).map_err(|err| {
                execution(
                    tool_id,
                    format!("luals config is not plain JSON (.jsonc is unsupported): {err}"),
                )
            })?)
        }
        None => None,
    };
    let luarc = work.join("luarc.json");
    std::fs::write(
        &luarc,
        commands::luals_luarc(
            consumer.as_ref(),
            library.as_deref(),
            &["dx-luals", ".dx-luals"],
        ),
    )
    .map_err(|err| execution(tool_id, format!("write luals config: {err}")))?;
    let report = work.join("check.json");
    let invocation = commands::luals_check(&tool.binary, &root, &work, &report, &luarc);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    // LuaLS exits 0 with findings, so only the report tells a run from a crash.
    let body = std::fs::read(&report).map_err(|err| {
        execution(
            tool_id,
            format!(
                "no report (exit {:?}): {err}: {}",
                out.code,
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ),
        )
    })?;
    let real_names: Vec<String> = owned
        .iter()
        .map(|(real, _)| real.to_string_lossy().into_owned())
        .collect();
    let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
    let mut findings = parsed(tool_id, parsers::parse_luals(&body, &real_strs))?;
    for found in &mut findings {
        if let Some((_, absolute)) = owned
            .iter()
            .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
        {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn phpstan(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let config = call.config;
    // PHPStan reports realpaths, so analyse and match the canonical ones.
    let root = std::fs::canonicalize(scratch.root())
        .map_err(|err| execution(tool_id, format!("canonicalize scratch root: {err}")))?;
    let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
    for (workspace, absolute) in pairs {
        let real = std::fs::canonicalize(absolute)
            .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
        owned.push((real, absolute));
    }
    let work = root.join(".dx-phpstan");
    let tmp_dir = work.join("tmp");
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|err| execution(tool_id, format!("stage phpstan tmp: {err}")))?;
    let scan_dirs: Vec<PathBuf> = ["dx-phpstan/scan", "dx-phpstan/pkg"]
        .iter()
        .filter(|dir| {
            tool.tool_files
                .iter()
                .any(|(rel, _)| rel.starts_with(&format!("{dir}/")))
        })
        .map(|dir| root.join(dir))
        .collect();
    let scan_refs: Vec<&Path> = scan_dirs.iter().map(PathBuf::as_path).collect();
    let consumer = match config {
        Some(path) => Some(
            std::fs::canonicalize(path)
                .map_err(|err| execution(tool_id, format!("resolve phpstan config: {err}")))?,
        ),
        None => None,
    };
    let extension = tool
        .tool_files
        .iter()
        .any(|(rel, _)| rel.starts_with("dx-phpstan/ext/"))
        .then(|| root.join("dx-phpstan/ext"));
    let autoload = extension.as_ref().map(|dir| dir.join("autoload.php"));
    let neon = work.join("dx.neon");
    std::fs::write(
        &neon,
        commands::phpstan_neon(
            consumer.as_deref(),
            PHPSTAN_LEVEL,
            &tmp_dir,
            &scan_refs,
            extension.as_deref(),
        ),
    )
    .map_err(|err| execution(tool_id, format!("write phpstan config: {err}")))?;
    let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
    let invocation = commands::phpstan_check(&tool.binary, &neon, autoload.as_deref(), &path_refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let real_names: Vec<String> = owned
        .iter()
        .map(|(real, _)| real.to_string_lossy().into_owned())
        .collect();
    let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
    let mut findings = parsed(
        tool_id,
        parsers::parse_phpstan(&out.stdout, out.code, &real_strs),
    )?;
    if call.capability == "unused" {
        findings = parsers::keep_unused_public(findings);
    }
    for found in &mut findings {
        if let Some((_, absolute)) = owned
            .iter()
            .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
        {
            found.file = absolute.to_string_lossy().into_owned();
        }
    }
    Ok(findings)
}

fn sorbet(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    // The owned sources and the staged dependency sources go on the command line, so
    // Sorbet resolves constants across them; only findings in the owned sources are
    // kept. It reads no config and writes no cache, so nothing outside the scratch
    // directory is touched.
    let mut deps: Vec<PathBuf> = tool
        .tool_files
        .iter()
        .filter(|(rel, _)| rel.starts_with("dx-sorbet/dep/"))
        .map(|(rel, _)| scratch.root().join(rel))
        .collect();
    deps.sort();
    let mut inputs: Vec<&Path> = refs.clone();
    inputs.extend(deps.iter().map(PathBuf::as_path));
    let invocation = commands::sorbet_check(&tool.binary, &inputs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    // Sorbet prints its errors on stderr.
    parsed(tool_id, parsers::parse_sorbet(&out.stderr, out.code, &strs))
}

fn perltidy(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    // One process per file: --standard-output takes a single file.
    let mut findings = Vec::new();
    for (workspace, absolute) in pairs {
        let invocation = commands::perltidy_check(&tool.binary, absolute.as_path());
        let out = backend.run(tool_id, tool, &invocation, scratch)?;
        let input = std::fs::read(absolute)
            .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
        let name = absolute.to_string_lossy();
        let found = parsed(
            tool_id,
            parsers::parse_perltidy(&out.stdout, &out.stderr, out.code, &input, &name),
        )?;
        findings.extend(found);
    }
    Ok(findings)
}

fn elvis(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let config = call.config;
    let (_, names) = paths(call);
    let Some(config) = config else {
        return Err(execution(tool_id, "elvis needs a config path".to_owned()));
    };
    let root = scratch.root();
    let relative = pairs
        .iter()
        .map(|(_, absolute)| {
            absolute.strip_prefix(root).map_err(|_| {
                execution(
                    tool_id,
                    format!("{} is outside the scratch", absolute.display()),
                )
            })
        })
        .collect::<Result<Vec<&Path>, RunnerError>>()?;
    let relative_names: Vec<String> = relative
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let relative_strs: Vec<&str> = relative_names.iter().map(String::as_str).collect();
    let invocation = commands::elvis_check(&tool.binary, &relative, config);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let mut findings = parsed(
        tool_id,
        parsers::parse_elvis(&out.stdout, &out.stderr, out.code, &relative_strs),
    )?;
    for found in &mut findings {
        if let Some(at) = relative_names.iter().position(|name| *name == found.file) {
            found.file.clone_from(&names[at]);
        }
    }
    Ok(findings)
}

fn erlfmt(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let invocation = commands::erlfmt_check(&tool.binary, &refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    parsed(
        tool_id,
        parsers::parse_erlfmt(&out.stdout, &out.stderr, out.code, &strs),
    )
}

fn credo(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let config = call.config;
    let (refs, names) = paths(call);
    let cwd_rel = RealBackend::cwd_rel(tool_id, tool.config_rel.as_deref());
    let Some(config) = config else {
        return Err(execution(tool_id, "credo needs a config path".to_owned()));
    };
    // Credo prints paths relative to its working directory, so it gets them that way.
    let cwd = scratch.root().join(&cwd_rel);
    let relative = refs
        .iter()
        .map(|absolute| {
            absolute.strip_prefix(&cwd).map_err(|_| {
                execution(
                    tool_id,
                    format!("{} is outside {}", absolute.display(), cwd.display()),
                )
            })
        })
        .collect::<Result<Vec<&Path>, RunnerError>>()?;
    let relative_names: Vec<String> = relative
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let relative_strs: Vec<&str> = relative_names.iter().map(String::as_str).collect();
    let invocation = commands::credo_check(&tool.binary, &relative, config);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let mut findings = parsed(
        tool_id,
        parsers::parse_credo(&out.stdout, &out.stderr, out.code, &relative_strs),
    )?;
    for found in &mut findings {
        if let Some(at) = relative_names.iter().position(|name| *name == found.file) {
            found.file.clone_from(&names[at]);
        }
    }
    Ok(findings)
}

fn mix_format(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let cwd_rel = RealBackend::cwd_rel(tool_id, tool.config_rel.as_deref());
    let invocation = commands::mix_format_check(&tool.binary, &refs);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    let cwd = scratch.root().join(&cwd_rel);
    parsed(
        tool_id,
        parsers::parse_mix_format(&out.stdout, &out.stderr, out.code, &strs, &cwd),
    )
}

fn selene(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let config = call.config;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let invocation = commands::selene_check(&tool.binary, &refs, config);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    parsed(tool_id, parsers::parse_selene(&out.stdout, out.code, &strs))
}

fn stylua(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let config = call.config;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let invocation = commands::stylua_check(&tool.binary, &refs, config);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    parsed(
        tool_id,
        parsers::parse_stylua(&out.stdout, &out.stderr, out.code, &strs),
    )
}

fn ormolu(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    // One process per file: ormolu formats files concurrently and interleaves
    // their diffs on stderr, and exits 102 on mixed results.
    let mut merged = Vec::new();
    for (name, path) in strs.iter().zip(refs.iter()) {
        let invocation = commands::ormolu_check(&tool.binary, std::slice::from_ref(path));
        let out = backend.run(tool_id, tool, &invocation, scratch)?;
        let found = parsed(
            tool_id,
            parsers::parse_ormolu(&out.stdout, &out.stderr, out.code, &[*name]),
        )
        .map_err(|err| in_file(err, name))?;
        merged.extend(found);
    }
    Ok(merged)
}

fn standardrb(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let config = call.config;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let invocation = commands::standardrb_check(&tool.binary, &refs, config);
    let out = backend.run(tool_id, tool, &invocation, scratch)?;
    parsed(
        tool_id,
        parsers::parse_standardrb(&out.stdout, &out.stderr, out.code, &strs, scratch.root()),
    )
}

fn lint_family(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let tool = call.tool;
    let scratch = &call.staged.scratch;
    let pairs = &call.staged.pairs;
    let config = call.config;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    if !tool.upstream_diagnostics.is_empty() {
        backend.check_file_family_delegated(tool_id, tool, pairs)
    } else {
        let invocation = match tool_id {
            "rubocop" => commands::rubocop_check(&tool.binary, &refs, config),
            "yamllint" => commands::yamllint_check(&tool.binary, &refs, config),
            "checkmake" => {
                let Some(config) = config else {
                    return Err(execution(
                        tool_id,
                        "checkmake needs a config path".to_owned(),
                    ));
                };
                commands::checkmake_check(&tool.binary, &refs, config)
            }
            "hadolint" => {
                let Some(config) = config else {
                    return Err(execution(
                        tool_id,
                        "hadolint needs a config path".to_owned(),
                    ));
                };
                commands::hadolint_check(&tool.binary, &refs, config)
            }
            _ => commands::shellcheck_check(&tool.binary, &refs, config),
        };
        let out = backend.run(tool_id, tool, &invocation, scratch)?;
        let report = match tool_id {
            "rubocop" => {
                parsers::parse_rubocop(&out.stdout, &out.stderr, out.code, &strs, scratch.root())
            }
            "yamllint" => parsers::parse_yamllint(&out.stdout, out.code, &strs),
            "checkmake" => parsers::parse_checkmake(
                &out.stdout,
                &out.stderr,
                out.code,
                &strs,
                config.and_then(std::path::Path::to_str),
            ),
            "hadolint" => parsers::parse_hadolint(&out.stdout, &out.stderr, out.code, &strs),
            _ => parsers::parse_shellcheck(&out.stdout, out.code, &strs),
        };
        parsed(tool_id, report)
    }
}
