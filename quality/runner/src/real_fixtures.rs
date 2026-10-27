use std::ffi::OsStr;

use super::*;
use crate::{FileInput, StageSpec};

pub(super) type Spawn = SpawnFn;

/// Escapes a path for embedding in a JSON string literal.
pub(super) fn json_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for char in raw.chars() {
        match char {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(char),
        }
    }
    out
}

pub(super) const BUILDIFIER_MIXED: &str = r#"{"success":false,"files":[{"filename":"FILE","formatted":false,"valid":true,"warnings":[{"start":{"line":1,"column":1},"end":{"line":1,"column":2},"category":"module-docstring","message":"The file has no module docstring."}]}]}"#;
pub(super) const BUILDIFIER_FAR: &str = r#"{"success":false,"files":[{"filename":"FILE","formatted":true,"valid":true,"warnings":[{"start":{"line":99,"column":1},"end":{"line":99,"column":2},"category":"module-docstring","message":"Far away."}]}]}"#;
pub(super) const TAPLO_BLOCK: &str = "error: invalid TOML\n  \u{250c}\u{2500} FILE:2:5\n  \u{2502}  \n2 \u{2502}   b = \n  \u{2502} \u{256d}\u{2500}\u{2500}\u{2500}\u{2500}^\n  \u{2502} \u{2570}^ expected value\n";
pub(super) const VALE_ALERT: &str = r#"{"FILE": [{"Action": {"Name": "", "Params": null}, "Span": [5, 10], "Check": "Test.Cotton", "Description": "", "Link": "", "Message": "Avoid cotton.", "Severity": "error", "Match": "cotton", "Line": 1}]}"#;
pub(super) const RUFF_F401: &str = r#"[{"cell":null,"code":"F401","end_location":{"column":10,"row":1},"filename":"FILE","fix":{"applicability":"safe","edits":[],"message":"Remove unused import"},"location":{"column":8,"row":1},"message":"`os` imported but unused","name":"unused-import","noqa_row":1,"severity":"error","url":"https://docs.astral.sh/ruff/rules/unused-import"}]"#;
pub(super) const RUFF_UNFORMATTED: &str = r#"[{"cell":null,"code":"unformatted","end_location":{"column":3,"row":1},"filename":"FILE","fix":null,"location":{"column":3,"row":1},"message":"File would be reformatted","name":"unformatted","noqa_row":null,"severity":"error","url":null}]"#;
pub(super) const DELEGATED_CLIPPY_WARN: &str = r#"{"$message_type":"artifact","artifact":"bazel-out/k8-fastbuild/bin/src/lib-123.d","emit":"dep-info"}
{"$message_type":"diagnostic","message":"length comparison to zero","code":{"code":"clippy::len_zero","explanation":null},"level":"warning","spans":[{"file_name":"src/main.rs","byte_start":8,"byte_end":20,"line_start":1,"line_end":1,"column_start":9,"column_end":21,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"use is_empty","code":null,"level":"help","spans":[{"file_name":"src/main.rs","byte_start":8,"byte_end":20,"line_start":1,"line_end":1,"column_start":9,"column_end":21,"is_primary":true,"text":[],"label":null,"suggested_replacement":"!v.is_empty()","suggestion_applicability":"MachineApplicable","expansion":null}],"children":[],"rendered":null}],"rendered":null}
{"$message_type":"diagnostic","message":"1 warning emitted","code":null,"level":"warning","spans":[],"children":[],"rendered":null}"#;

pub(super) const DELEGATED_RUSTC_WARN: &str = r#"{"$message_type":"artifact","artifact":"bazel-out/k8-fastbuild/bin/dx/qual/libdx_qual-1134879743.rlib","emit":"link"}
{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"error","spans":[{"file_name":"src/main.rs","byte_start":24,"byte_end":29,"line_start":2,"line_end":2,"column_start":13,"column_end":17,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}
{"$message_type":"diagnostic","message":"aborting due to previous error","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

pub(super) fn plain_tool() -> RealTool {
    RealTool {
        binary: PathBuf::from("/fake/bin/tool"),
        extra_env: Vec::new(),
        config_rel: None,
        edition: None,
        tool_files: Vec::new(),
        upstream_diagnostics: Vec::new(),
        search_paths: Vec::new(),
    }
}

pub(super) fn rustfmt_tool() -> RealTool {
    RealTool {
        edition: Some("2021".to_owned()),
        ..plain_tool()
    }
}

pub(super) fn upstream_file(name: &str, body: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .prefix(format!("dx-delegated-{name}-").as_str())
        .tempfile_in(std::env::temp_dir())
        .expect("claim upstream fixture");
    std::io::Write::write_all(&mut file, body.as_bytes())
        .expect("write upstream diagnostics fixture");
    file
}

pub(super) fn delegated_tool(path: PathBuf) -> RealTool {
    RealTool {
        binary: PathBuf::new(),
        tool_files: vec![("dx-roslyn/exit".to_owned(), b"0\n".to_vec())],
        upstream_diagnostics: vec![path],
        ..plain_tool()
    }
}

pub(super) fn backend_for(tool_id: &str, tool: RealTool, spawn: Spawn) -> RealBackend {
    let mut tools = BTreeMap::new();
    tools.insert(tool_id.to_owned(), tool);
    RealBackend {
        tools,
        scratch_parent: std::env::temp_dir(),
        spawn,
        argv_limit: quality_adapter::commands::windows_argv_limit,
    }
}

pub(super) fn single(path: &str, text: &str) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert(path.to_owned(), text.to_owned());
    files
}

pub(super) fn stage(tool: &str, classes: &[&str], sources: &[&str]) -> StageSpec {
    StageSpec {
        tool_id: tool.to_owned(),
        class_ids: classes.iter().map(ToString::to_string).collect(),
        source_paths: sources.iter().map(ToString::to_string).collect(),
    }
}

pub(super) fn file(path: &str, body: &str) -> FileInput {
    FileInput {
        path: path.to_owned(),
        bytes: body.as_bytes().to_vec(),
    }
}

pub(super) fn assert_hermetic(env: &[(OsString, OsString)]) {
    let folded = |key: &OsString| key.to_string_lossy().to_ascii_uppercase();
    let pinned = |key: &str| {
        let hits: Vec<&OsString> = env
            .iter()
            .filter(|(known, _)| known == key)
            .map(|(_, value)| value)
            .collect();
        assert_eq!(hits.len(), 1, "exactly one {key}");
        hits[0].clone()
    };
    let tmpdir = pinned("TMPDIR");
    assert!(!tmpdir.is_empty(), "TMPDIR names a scratch tree");
    assert_eq!(pinned("TEMP"), tmpdir, "TEMP matches TMPDIR");
    assert_eq!(pinned("TMP"), tmpdir, "TMP matches TMPDIR");
    assert!(
        !env.iter().any(|(key, _)| folded(key) == "PATH"),
        "PATH is never set"
    );
    for denied in [
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "HOMEDRIVE",
        "HOMEPATH",
        "PUBLIC",
        "ALLUSERSPROFILE",
        "PROGRAMDATA",
        "USERNAME",
        "PYTHONPATH",
        "PYTHONHOME",
        "NODE_OPTIONS",
        "NODE_PATH",
    ] {
        assert!(
            env.iter().all(|(key, _)| folded(key) != denied),
            "{denied} never leaks"
        );
    }
    assert_eq!(pinned("LANG"), OsString::from("C.UTF-8"));
    assert_eq!(pinned("TZ"), OsString::from("UTC"));
}

pub(super) fn last_file(argv: &[OsString]) -> String {
    argv.last()
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn trim_end(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b' ' || line[end - 1] == b'\t') {
        end -= 1;
    }
    &line[..end]
}

pub(super) fn roundtrip_rustfmt(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    if argv.iter().any(|arg| arg == "--check") {
        let dirty = bytes
            .split(|byte| *byte == b'\n')
            .any(|line| line.ends_with(b" ") || line.ends_with(b"\t"));
        if dirty {
            let stdout = format!("Diff in {file}:1:\n-x  \n+x\n");
            return Ok(ChildOutput {
                code: Some(1),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            });
        }
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    let mut fixed = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let trailing = line.ends_with(b"\n");
        let body = if trailing {
            &line[..line.len() - 1]
        } else {
            line
        };
        fixed.extend_from_slice(trim_end(body));
        if trailing {
            fixed.push(b'\n');
        }
    }
    std::fs::write(&file, fixed).expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn module_tree(file: &Path, tree: &mut Vec<PathBuf>) -> Result<(), String> {
    if tree.iter().any(|known| known == file) {
        return Ok(());
    }
    tree.push(file.to_path_buf());
    let text = std::fs::read_to_string(file).expect("module is materialized");
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let dir = file.parent().unwrap_or(Path::new(""));
    let base = if matches!(stem.as_ref(), "lib" | "main" | "mod") {
        dir.to_path_buf()
    } else {
        dir.join(stem.as_ref())
    };
    let mut path_attr: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(value) = line
            .strip_prefix("#[path = \"")
            .and_then(|rest| rest.strip_suffix("\"]"))
        {
            path_attr = Some(value.to_owned());
            continue;
        }
        let attr = path_attr.take();
        let Some(name) = line
            .strip_prefix("mod ")
            .or_else(|| line.strip_prefix("pub mod "))
            .and_then(|rest| rest.strip_suffix(';'))
        else {
            continue;
        };
        let flat = match attr {
            Some(value) => dir.join(value),
            None => base.join(format!("{name}.rs")),
        };
        if !flat.is_file() {
            return Err(format!(
                "failed to resolve mod `{name}`: {} does not exist",
                flat.display()
            ));
        }
        module_tree(&flat, tree)?;
    }
    Ok(())
}

pub(super) fn module_files(argv: &[OsString]) -> Result<Vec<PathBuf>, String> {
    let mut tree = Vec::new();
    for arg in argv.iter().skip(1) {
        let path = Path::new(arg);
        if path.extension() == Some(OsStr::new("rs")) {
            module_tree(path, &mut tree)?;
        }
    }
    Ok(tree)
}

fn has_trailing_space(bytes: &[u8]) -> bool {
    bytes
        .split(|byte| *byte == b'\n')
        .any(|line| line.ends_with(b" ") || line.ends_with(b"\t"))
}

/// Formats every file and resolvable module like rustfmt and fails on missing modules.
pub(super) fn module_rustfmt(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let tree = match module_files(argv) {
        Ok(tree) => tree,
        Err(message) => {
            return Ok(ChildOutput {
                code: Some(1),
                stdout: Vec::new(),
                stderr: format!("Error writing files: {message}\n").into_bytes(),
            });
        }
    };
    let check = argv.iter().any(|arg| arg == "--check");
    let mut stdout = String::new();
    for file in &tree {
        let bytes = std::fs::read(file).expect("module is materialized");
        if !has_trailing_space(&bytes) {
            continue;
        }
        if check {
            stdout.push_str(&format!("Diff in {}:1:\n-x  \n+x\n", file.display()));
            continue;
        }
        let mut fixed = Vec::with_capacity(bytes.len());
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            let body = line.strip_suffix(b"\n").unwrap_or(line);
            fixed.extend_from_slice(trim_end(body));
            if line.ends_with(b"\n") {
                fixed.push(b'\n');
            }
        }
        std::fs::write(file, fixed).expect("fix writes back");
    }
    Ok(ChildOutput {
        code: Some(if stdout.is_empty() { 0 } else { 1 }),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

/// Fails a fix whose staged child modules carry any bytes.
pub(super) fn placeholder_children_rustfmt(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    if !argv.iter().any(|arg| arg == "--check") {
        let tree = module_files(argv).unwrap_or_default();
        for child in tree.iter().skip(1) {
            if std::fs::metadata(child)
                .expect("module is materialized")
                .len()
                > 0
            {
                return Ok(ChildOutput {
                    code: Some(2),
                    stdout: Vec::new(),
                    stderr: format!("child with bytes {}\n", child.display()).into_bytes(),
                });
            }
        }
    }
    module_rustfmt(argv, cwd, env)
}

/// Reports an unclosed delimiter for every module containing `( {`.
pub(super) fn unclosed_rustfmt(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let tree = module_files(argv).unwrap_or_default();
    let mut stderr = String::new();
    for file in &tree {
        let text = std::fs::read_to_string(file).expect("module is materialized");
        if text.contains("( {") {
            stderr.push_str(&format!(
                "error: this file contains an unclosed delimiter\n --> {}:1:4\n",
                file.display()
            ));
        }
    }
    if stderr.is_empty() {
        return module_rustfmt(argv, cwd, env);
    }
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

pub(super) fn buildifier_plain(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let stdout = BUILDIFIER_MIXED.replace("FILE", &json_escape(&last_file(argv)));
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) const BUILDIFIER_INVALID: &str =
    r#"{"files":[{"filename":"FILE","formatted":false,"valid":false,"warnings":[]}]}"#;

pub(super) fn buildifier_invalid(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let stdout = BUILDIFIER_INVALID.replace("FILE", &json_escape(&file));
    Ok(ChildOutput {
        code: Some(4),
        stdout: stdout.into_bytes(),
        stderr: format!("{file}:3:1: syntax error near unexpected token\n").into_bytes(),
    })
}

pub(super) fn buildifier_hinted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        cwd.ends_with("cfg"),
        "hinted buildifier runs from the config dir"
    );
    buildifier_plain(argv, cwd, env)
}

pub(super) fn buildifier_far(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let stdout = BUILDIFIER_FAR.replace("FILE", &json_escape(&last_file(argv)));
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn garbage_stdout(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Ok(ChildOutput {
        code: Some(0),
        stdout: b"[]".to_vec(),
        stderr: Vec::new(),
    })
}

pub(super) const BIOME_LINT_DIRTY: &str = r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[{"severity":"warning","message":"This variable unusedVar is unused.","category":"lint/correctness/noUnusedVariables","location":{"path":"FILE","start":{"line":1,"column":7},"end":{"line":1,"column":16}},"advices":[]}],"command":"lint"}"#;
pub(super) const BIOME_LINT_CLEAN: &str =
    r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[],"command":"lint"}"#;
pub(super) const BIOME_FMT_DIRTY: &str = r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[{"severity":"error","message":"Formatter would have printed the following content:","category":"format","location":{"path":"FILE","start":{"line":0,"column":0},"end":{"line":0,"column":0}},"advices":[]}],"command":"format"}"#;
pub(super) const BIOME_FMT_CLEAN: &str = r#"{"summary":{},"diagnostics":[],"command":"format"}"#;
pub(super) const ESLINT_DIRTY: &str = r#"[{"filePath":"FILE","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"'unusedVar' is assigned a value but never used.","line":1,"column":7,"endLine":1,"endColumn":16}],"errorCount":1,"warningCount":0}]"#;
pub(super) const KEEP_SORTED_DIRTY: &str = "# keep-sorted start\nc\nb\na\n# keep-sorted end\n";
pub(super) const KEEP_SORTED_CLEAN: &str = "# keep-sorted start\na\nb\nc\n# keep-sorted end\n";
pub(super) const KEEP_SORTED_MESSAGE: &str = "These lines are out of order.";

fn keep_sorted_block(text: &str) -> Option<(usize, usize, Vec<&str>)> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == "# keep-sorted start")?;
    let end = lines.iter().position(|line| *line == "# keep-sorted end")?;
    if end <= start + 1 {
        return None;
    }
    Some((start, end, lines[start + 1..end].to_vec()))
}

pub(super) fn keep_sorted_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let mode = argv
        .windows(2)
        .find(|pair| pair[0] == "--mode")
        .map(|pair| pair[1].to_string_lossy().into_owned())
        .expect("keep-sorted always takes --mode");
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if mode == "fix" {
        if let Some((start, end, mut block)) = keep_sorted_block(&text) {
            block.sort();
            let mut lines: Vec<&str> = text.lines().collect();
            lines.splice(start + 1..end, block);
            let mut fixed = lines.join("\n");
            if text.ends_with('\n') {
                fixed.push('\n');
            }
            std::fs::write(&file, fixed).expect("fix writes back");
        }
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    assert_eq!(mode, "lint", "check stays a check");
    let Some((start, end, block)) = keep_sorted_block(&text) else {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    };
    let mut sorted = block.clone();
    sorted.sort();
    if sorted == block {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    let replacement = sorted.join("\n") + "\n";
    let stdout = format!(
        "[{{\"path\":\"{}\",\"lines\":{{\"start\":{},\"end\":{}}},\"message\":\"{}\",\"fixes\":[{{\"replacements\":[{{\"lines\":{{\"start\":{},\"end\":{}}},\"new_content\":\"{}\"}}]}}]}}]",
        json_escape(&file),
        start + 2,
        end,
        KEEP_SORTED_MESSAGE,
        start + 2,
        end,
        json_escape(&replacement),
    );
    Ok(ChildOutput {
        code: Some(1),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn keep_sorted_multi_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let files_at = argv
        .windows(2)
        .position(|pair| pair[0] == "--mode")
        .map(|at| at + 2)
        .expect("keep-sorted always takes --mode");
    assert_eq!(
        argv[files_at - 1].to_string_lossy(),
        "lint",
        "check stays a check"
    );
    let mut entries = Vec::new();
    for file in &argv[files_at..] {
        let name = file.to_string_lossy().into_owned();
        let bytes = std::fs::read(&name).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        let Some((start, end, block)) = keep_sorted_block(&text) else {
            continue;
        };
        let mut sorted = block.clone();
        sorted.sort();
        if sorted == block {
            continue;
        }
        let replacement = sorted.join("\n") + "\n";
        entries.push(format!(
            "{{\"path\":\"{}\",\"lines\":{{\"start\":{},\"end\":{}}},\"message\":\"{}\",\"fixes\":[{{\"replacements\":[{{\"lines\":{{\"start\":{},\"end\":{}}},\"new_content\":\"{}\"}}]}}]}}",
            json_escape(&name),
            start + 2,
            end,
            KEEP_SORTED_MESSAGE,
            start + 2,
            end,
            json_escape(&replacement),
        ));
    }
    if entries.is_empty() {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(1),
        stdout: format!("[{}]", entries.join(",")).into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn biome_config_dir_arg(argv: &[OsString]) -> String {
    argv.windows(2)
        .find(|pair| pair[0] == "--config-path")
        .map(|pair| pair[1].to_string_lossy().into_owned())
        .expect("--config-path is always passed")
}

pub(super) fn roundtrip_biome(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--reporter=json"),
        "biome reports JSON"
    );
    assert!(
        argv.iter().any(|arg| arg == "--colors=off"),
        "biome never emits color"
    );
    let config_dir = biome_config_dir_arg(argv);
    assert!(
        Path::new(config_dir.as_str()).join("biome.json").is_file(),
        "biome config dir holds exactly one biome.json"
    );
    let file = last_file(argv);
    let reported = Path::new(&file)
        .strip_prefix(cwd)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or(file.clone());
    if argv.get(1).map(OsString::as_os_str) == Some(OsStr::new("lint")) {
        assert!(
            argv.iter().any(|arg| arg == "--error-on-warnings"),
            "biome lint errors on warnings"
        );
        assert!(
            argv.iter().any(|arg| arg == "--vcs-enabled=false"),
            "biome lint never observes VCS state"
        );
        assert!(
            !argv.iter().any(|arg| arg == "--write"),
            "biome lint is check-only"
        );
        let bytes = std::fs::read(&file).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        if text.contains("unusedVar") {
            let stdout = BIOME_LINT_DIRTY.replace("FILE", &json_escape(&reported));
            return Ok(ChildOutput {
                code: Some(1),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            });
        }
        return Ok(ChildOutput {
            code: Some(0),
            stdout: BIOME_LINT_CLEAN.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }
    assert_eq!(
        argv.get(1).map(OsString::as_os_str),
        Some(OsStr::new("format")),
        "biome only runs lint or format"
    );
    if argv.iter().any(|arg| arg == "--write") {
        let bytes = std::fs::read(&file).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        let fixed = text.replace("BADFMT", "1");
        std::fs::write(&file, fixed).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("BADFMT") {
        let stdout = BIOME_FMT_DIRTY.replace("FILE", &json_escape(&reported));
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: BIOME_FMT_CLEAN.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

pub(super) fn biome_defaults(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config_dir = biome_config_dir_arg(argv);
    assert!(
        Path::new(config_dir.as_str()).ends_with("dx-biome-default"),
        "unhinted biome uses the materialized defaults dir"
    );
    let staged = std::fs::read(Path::new(config_dir.as_str()).join("biome.json"))
        .expect("defaults biome.json is staged");
    assert_eq!(staged, b"{}", "unhinted biome pins empty-object defaults");
    roundtrip_biome(argv, cwd, env)
}

pub(super) fn biome_hinted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config_dir = biome_config_dir_arg(argv);
    assert!(
        Path::new(config_dir.as_str()).ends_with("cfg"),
        "hinted biome resolves the hinted config parent"
    );
    let staged = std::fs::read(Path::new(config_dir.as_str()).join("biome.json"))
        .expect("hinted biome.json is staged");
    assert_eq!(
        staged, b"{\"linter\":{\"enabled\":false}}",
        "hinted biome stages the hinted bytes"
    );
    roundtrip_biome(argv, cwd, env)
}

pub(super) fn roundtrip_graphql_eslint(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let value = |flag: &str| {
        argv.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| PathBuf::from(&pair[1]))
            .expect("graphql_eslint takes --config and --root")
    };
    assert!(value("--config").is_file(), "config is materialized");
    assert!(value("--root").is_dir(), "root is the scratch directory");
    assert!(cwd.is_dir());
    let separator = argv
        .iter()
        .position(|arg| arg == "--")
        .expect("files follow --");
    let file = last_file(argv);
    assert_eq!(separator + 2, argv.len(), "one file after --");
    let text = std::fs::read_to_string(&file).expect("checked file is materialized");
    if text.contains("BROKEN") {
        return Ok(ChildOutput {
            code: Some(2),
            stdout: b"[]".to_vec(),
            stderr: b"graphql_eslint: schema pointer api/missing.graphql matches no file\n"
                .to_vec(),
        });
    }
    let messages = if text.contains("email") {
        r#"{"ruleId":"@graphql-eslint/fields-on-correct-type","severity":2,"message":"Cannot query field \"email\" on type \"User\".","line":1,"column":9,"endLine":1,"endColumn":14}"#
    } else {
        ""
    };
    Ok(ChildOutput {
        code: Some(i32::from(!messages.is_empty())),
        stdout: format!(
            r#"[{{"filePath":"{}","messages":[{messages}],"errorCount":0,"warningCount":0}}]"#,
            json_escape(&file)
        )
        .into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_eslint(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "-f") && argv.iter().any(|arg| arg == "json"),
        "eslint reports JSON"
    );
    let config = argv
        .windows(2)
        .find(|pair| pair[0] == "-c")
        .map(|pair| pair[1].to_string_lossy().into_owned())
        .expect("eslint always takes an explicit config");
    assert!(
        Path::new(config.as_str()).is_file(),
        "eslint config is materialized"
    );
    let file = last_file(argv);
    if argv.iter().any(|arg| arg == "--fix") {
        let bytes = std::fs::read(&file).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        let fixed = text.replace("unusedVar", "usedVar");
        std::fs::write(&file, fixed).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("unusedVar") {
        let stdout = ESLINT_DIRTY.replace("FILE", &json_escape(&file));
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    let stdout = format!(
        r#"[{{"filePath":"{}","messages":[],"errorCount":0,"warningCount":0}}]"#,
        json_escape(&file)
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_prettier(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        !argv.iter().any(|arg| arg == "--no-config"),
        "the prettier shim pins the config"
    );
    assert!(
        argv.iter().any(|arg| arg == "--no-editorconfig"),
        "prettier never observes editorconfig"
    );
    let file = last_file(argv);
    let reported = Path::new(&file)
        .strip_prefix(cwd)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or(file.clone());
    if argv.iter().any(|arg| arg == "--write") {
        let bytes = std::fs::read(&file).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        let fixed = text.replace("BADFMT", "1");
        std::fs::write(&file, fixed).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    assert!(
        argv.iter().any(|arg| arg == "--check"),
        "prettier check stays a check"
    );
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("BADFMT") {
        let stderr = format!(
            "[warn] {reported}\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n"
        );
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn missing_spawn(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Err(io::Error::new(io::ErrorKind::NotFound, "no such binary"))
}

pub(super) fn fatal_fix(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(2),
        stdout: Vec::new(),
        stderr: b"fatal error".to_vec(),
    })
}

pub(super) fn taplo_either(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    if argv.iter().any(|arg| arg == "--check") {
        let stderr = format!(
            "ERROR taplo:format_files: the file is not properly formatted path=\"{file}\"\n"
        );
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
        });
    }
    let stderr = TAPLO_BLOCK.replace("FILE", &file);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

pub(super) fn taplo_reads_disk(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    if argv.iter().any(|arg| arg == "--check") {
        let dirty = String::from_utf8_lossy(&bytes).contains("a=1");
        let stderr = if dirty {
            format!(
                "ERROR taplo:format_files: the file is not properly formatted path=\"{file}\"\n"
            )
            .into_bytes()
        } else {
            Vec::new()
        };
        return Ok(ChildOutput {
            code: Some(i32::from(dirty)),
            stdout: Vec::new(),
            stderr,
        });
    }
    std::fs::write(&file, b"a = 1\n").expect("formatted file is writable");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn taplo_garbage(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: b"unexpected taplo output".to_vec(),
    })
}

pub(super) fn assert_ruff_hermetic(argv: &[OsString], env: &[(OsString, OsString)]) {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--no-cache"),
        "ruff never caches"
    );
    assert!(
        argv.iter().any(|arg| arg == "--no-respect-gitignore"),
        "ruff never observes VCS state"
    );
}

pub(super) fn roundtrip_ruff(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_ruff_hermetic(argv, env);
    assert!(
        argv.iter().any(|arg| arg == "--isolated"),
        "unhinted ruff pins upstream defaults"
    );
    ruff_behavior(argv)
}

pub(super) fn roundtrip_ruff_hinted(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_ruff_hermetic(argv, env);
    assert!(
        argv.iter().any(|arg| arg == "--config"),
        "hinted ruff takes the hint"
    );
    assert!(
        !argv.iter().any(|arg| arg == "--isolated"),
        "hinted ruff never isolates"
    );
    ruff_behavior(argv)
}

pub(super) fn ruff_behavior(argv: &[OsString]) -> io::Result<ChildOutput> {
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("fix bytes stay UTF-8");
    if argv.iter().any(|arg| arg == "format") {
        if argv.iter().any(|arg| arg == "--check") {
            let dirty = text
                .lines()
                .any(|line| line.ends_with(' ') || line.ends_with('\t'));
            if dirty {
                let stdout = RUFF_UNFORMATTED.replace("FILE", &json_escape(&file));
                return Ok(ChildOutput {
                    code: Some(1),
                    stdout: stdout.into_bytes(),
                    stderr: Vec::new(),
                });
            }
            return Ok(ChildOutput {
                code: Some(0),
                stdout: b"[]".to_vec(),
                stderr: Vec::new(),
            });
        }
        let mut fixed = Vec::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            let trailing = line.ends_with('\n');
            let body = if trailing {
                &line[..line.len() - 1]
            } else {
                line
            };
            fixed.extend_from_slice(trim_end(body.as_bytes()));
            if trailing {
                fixed.push(b'\n');
            }
        }
        std::fs::write(&file, fixed).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    if argv.iter().any(|arg| arg == "--fix") {
        let kept: Vec<&str> = text
            .lines()
            .filter(|line| !line.contains("import os"))
            .collect();
        let mut fixed = kept.join("\n");
        if text.ends_with('\n') {
            fixed.push('\n');
        }
        std::fs::write(&file, fixed.clone()).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(i32::from(fixed.contains("UNFIXABLE"))),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    if text.contains("import os") {
        let stdout = RUFF_F401.replace("FILE", &json_escape(&file));
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: b"[]".to_vec(),
        stderr: Vec::new(),
    })
}

/// Ruff over every `.py` file in argv, the way a batched spawn hands them over.
pub(super) fn ruff_multi_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_ruff_hermetic(argv, env);
    let format = argv.iter().any(|arg| arg == "format");
    let mut entries = Vec::new();
    for file in argv
        .iter()
        .filter(|arg| arg.to_string_lossy().ends_with(".py"))
    {
        let name = file.to_string_lossy().into_owned();
        let bytes = std::fs::read(&name).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        let template = if format {
            let dirty = text
                .lines()
                .any(|line| line.ends_with(' ') || line.ends_with('\t'));
            dirty.then_some(RUFF_UNFORMATTED)
        } else {
            text.contains("import os").then_some(RUFF_F401)
        };
        if let Some(template) = template {
            let entry = template.replace("FILE", &json_escape(&name));
            entries.push(entry[1..entry.len() - 1].to_owned());
        }
    }
    Ok(ChildOutput {
        code: Some(i32::from(!entries.is_empty())),
        stdout: format!("[{}]", entries.join(",")).into_bytes(),
        stderr: Vec::new(),
    })
}

/// Flake8 over every `.py` file in argv, the way a batched spawn hands them over.
pub(super) fn flake8_multi_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let mut stdout = String::new();
    for file in argv
        .iter()
        .filter(|arg| arg.to_string_lossy().ends_with(".py"))
    {
        let name = file.to_string_lossy().into_owned();
        let bytes = std::fs::read(&name).expect("checked file is materialized");
        let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
        if text.contains("import os") {
            stdout.push_str(&format!("{name}:1:1:F401:'os' imported but unused\n"));
        }
    }
    Ok(ChildOutput {
        code: Some(i32::from(!stdout.is_empty())),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_ty(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--no-respect-ignore-files"),
        "ty never observes VCS state"
    );
    let file = last_file(argv);
    let reported = Path::new(&file)
        .strip_prefix(cwd)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or(file.clone());
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("BADTYPE") {
        let stdout = format!(
            "{reported}:1:10: error[invalid-assignment] Object of type `Literal[\"hello\"]` is not assignable to `int`\nFound 1 diagnostic\n"
        );
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: b"All checks passed!\n".to_vec(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_pydoclint(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--quiet"),
        "pydoclint stays quiet"
    );
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("NODOC") {
        let stderr = format!(
            "{file}\n    2: DOC201: Function `foo` does not have a return section in docstring\n"
        );
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_flake8(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--isolated"),
        "flake8 pins upstream defaults"
    );
    assert!(
        argv.iter().any(|arg| arg == "--jobs=1"),
        "flake8 keeps output order deterministic"
    );
    assert!(
        argv.iter().any(|arg| arg == "--color=never"),
        "flake8 never emits color"
    );
    let file = last_file(argv);
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("import os") {
        let stdout = format!("{file}:1:1:F401:'os' imported but unused\n");
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_pylint(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--persistent=n"),
        "pylint never caches"
    );
    assert!(
        argv.iter().any(|arg| arg == "--reports=n"),
        "pylint suppresses the report"
    );
    assert!(
        argv.iter().any(|arg| arg == "--score=n"),
        "pylint suppresses the score"
    );
    assert!(
        argv.iter().any(|arg| arg == "--output-format=json"),
        "pylint reports JSON"
    );
    let file = last_file(argv);
    let reported = Path::new(&file)
        .strip_prefix(cwd)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or(file.clone());
    let bytes = std::fs::read(&file).expect("checked file is materialized");
    let text = String::from_utf8(bytes).expect("checked bytes stay UTF-8");
    if text.contains("import os") {
        let stdout = format!(
            "[{{\"type\": \"warning\", \"module\": \"a\", \"obj\": \"\", \"line\": 1, \"column\": 0, \"endLine\": 1, \"endColumn\": 9, \"path\": \"{}\", \"symbol\": \"unused-import\", \"message\": \"Unused import os\", \"message-id\": \"W0611\"}}]",
            json_escape(&reported)
        );
        return Ok(ChildOutput {
            code: Some(4),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: b"[]".to_vec(),
        stderr: Vec::new(),
    })
}

pub(super) fn vale_hinted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        cwd.ends_with("vdir"),
        "hinted vale runs from the config dir"
    );
    let stdout = VALE_ALERT.replace("FILE", &json_escape(&last_file(argv)));
    Ok(ChildOutput {
        code: Some(1),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}
