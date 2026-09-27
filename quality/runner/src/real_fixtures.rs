use super::*;

pub(super) type Spawn = SpawnFn;

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

pub(super) fn assert_hermetic(env: &[(String, String)]) {
    assert!(
        env.iter().any(|(key, _)| key == "TMPDIR"),
        "TMPDIR is always set"
    );
    assert!(
        !env.iter().any(|(key, _)| key == "PATH"),
        "PATH is never set"
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "LANG" && value == "C.UTF-8"),
        "LANG is pinned to C.UTF-8"
    );
    assert!(
        env.iter().any(|(key, value)| key == "TZ" && value == "UTC"),
        "TZ is pinned to UTC"
    );
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
    env: &[(String, String)],
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

pub(super) fn buildifier_plain(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let stdout = BUILDIFIER_MIXED.replace("FILE", &last_file(argv));
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn buildifier_hinted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(String, String)],
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
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let stdout = BUILDIFIER_FAR.replace("FILE", &last_file(argv));
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn garbage_stdout(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(String, String)],
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

pub(super) fn biome_config_dir_arg(argv: &[OsString]) -> String {
    argv.windows(2)
        .find(|pair| pair[0] == "--config-path")
        .map(|pair| pair[1].to_string_lossy().into_owned())
        .expect("--config-path is always passed")
}

pub(super) fn roundtrip_biome(
    argv: &[OsString],
    cwd: &Path,
    env: &[(String, String)],
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
            let stdout = BIOME_LINT_DIRTY.replace("FILE", &reported);
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
        let stdout = BIOME_FMT_DIRTY.replace("FILE", &reported);
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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

pub(super) fn roundtrip_eslint(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(String, String)],
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
        let stdout = ESLINT_DIRTY.replace("FILE", &file);
        return Ok(ChildOutput {
            code: Some(1),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        });
    }
    let stdout =
        format!(r#"[{{"filePath":"{file}","messages":[],"errorCount":0,"warningCount":0}}]"#);
    Ok(ChildOutput {
        code: Some(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn roundtrip_prettier(
    argv: &[OsString],
    cwd: &Path,
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--no-config"),
        "prettier never observes config files"
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
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Err(io::Error::new(io::ErrorKind::NotFound, "no such binary"))
}

pub(super) fn fatal_fix(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(String, String)],
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
    env: &[(String, String)],
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

pub(super) fn taplo_garbage(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: b"unexpected taplo output".to_vec(),
    })
}

pub(super) fn assert_ruff_hermetic(argv: &[OsString], env: &[(String, String)]) {
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
                let stdout = RUFF_UNFORMATTED.replace("FILE", &file);
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
        let stdout = RUFF_F401.replace("FILE", &file);
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

pub(super) fn roundtrip_ty(
    argv: &[OsString],
    cwd: &Path,
    env: &[(String, String)],
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
            "[{{\"type\": \"warning\", \"module\": \"a\", \"obj\": \"\", \"line\": 1, \"column\": 0, \"endLine\": 1, \"endColumn\": 9, \"path\": \"{reported}\", \"symbol\": \"unused-import\", \"message\": \"Unused import os\", \"message-id\": \"W0611\"}}]"
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
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        cwd.ends_with("vdir"),
        "hinted vale runs from the config dir"
    );
    let stdout = VALE_ALERT.replace("FILE", &last_file(argv));
    Ok(ChildOutput {
        code: Some(1),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}
