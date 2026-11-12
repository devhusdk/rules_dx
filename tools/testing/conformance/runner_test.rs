#[test]
fn subject_matches_mode() {
    let mode = std::env::var("DX_CONFORMANCE_MODE").expect("DX_CONFORMANCE_MODE must be set");
    let key = std::env::var("DX_CONFORMANCE_SUBJECT").expect("DX_CONFORMANCE_SUBJECT must be set");
    let text = std::env::var("DX_CONFORMANCE_TEXT").unwrap_or_default();
    let pattern = std::env::var("DX_CONFORMANCE_PATTERN").unwrap_or_else(|_| "{case}".to_owned());
    if mode == "cross" {
        assert_image(&key, &text);
        return;
    }
    if ["lint", "lint_error", "format", "format_error"].contains(&mode.as_str()) {
        conformance_quality::check(&mode, &key, &text);
        return;
    }
    let mut subject = None;
    let mut subject_env = Vec::new();
    let mut subject_key = String::new();
    let mut manifest = String::new();
    for entry in key.split(' ').filter(|entry| !entry.is_empty()) {
        let path = dx_testing::rlocation(entry).expect("subject must resolve from runfiles");
        if entry.ends_with(".coverage_manifest") {
            manifest = path.to_string_lossy().into_owned();
        } else if entry.ends_with(".env") {
            let text = std::fs::read_to_string(&path).expect("subject env must be readable");
            for line in text.lines() {
                if let Some((name, value)) = line.split_once('=') {
                    subject_env.push((name.to_owned(), value.to_owned()));
                }
            }
        } else {
            subject = Some(path);
            subject_key = entry.to_owned();
        }
    }
    let subject = subject.expect("subject must list an executable");
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set");
    let subject = stage(&subject, &subject_key, std::path::Path::new(&tmp));
    let srcdir = std::env::var("TEST_SRCDIR").unwrap_or_default();
    let runfiles = std::env::var("RUNFILES_DIR").unwrap_or_else(|_| srcdir.clone());
    let mut envs = vec![
        ("TEST_TMPDIR", tmp.as_str()),
        ("TEST_SRCDIR", srcdir.as_str()),
        ("RUNFILES_DIR", runfiles.as_str()),
    ];
    let cov = std::path::Path::new(&tmp).join("cov");
    let raw = cov.join("raw");
    let out = cov.join("coverage.dat");
    let (raw_s, out_s) = (
        raw.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
    );
    if mode == "coverage" {
        std::fs::create_dir_all(&raw).expect("coverage dir must be creatable");
        envs.push(("COVERAGE", "1"));
        envs.push(("COVERAGE_DIR", raw_s.as_str()));
        envs.push(("COVERAGE_OUTPUT_FILE", out_s.as_str()));
        envs.push(("COVERAGE_MANIFEST", manifest.as_str()));
        envs.push(("TEST_WORKSPACE", "_main"));
    }
    let subject_env: Vec<(String, String)> = subject_env
        .into_iter()
        .map(|(name, value)| {
            let value = if mode == "coverage" {
                collector_path(&value)
            } else {
                value
            };
            (name, value)
        })
        .collect();
    for (name, value) in &subject_env {
        envs.push((name.as_str(), value.as_str()));
    }
    let native = subject_env
        .iter()
        .any(|(name, _)| name == "CC_CODE_COVERAGE_SCRIPT");
    let source_root = if native {
        runfiles.clone()
    } else {
        std::path::Path::new(&runfiles)
            .join("_main")
            .to_string_lossy()
            .into_owned()
    };
    if mode == "coverage" {
        envs.push(("ROOT", source_root.as_str()));
    }
    let profile = raw.join("%h-%p-%m.profraw").to_string_lossy().into_owned();
    let binary = subject_key
        .split_once('/')
        .map_or("", |(_, rest)| rest)
        .to_owned();
    if mode == "coverage" && native {
        envs.push(("LLVM_PROFILE_FILE", profile.as_str()));
        envs.push(("TEST_BINARY", binary.as_str()));
    }
    let subject_args: Vec<String> = std::env::var("DX_CONFORMANCE_ARGS")
        .map(|args| args.split(' ').map(str::to_owned).collect())
        .unwrap_or_default();
    let subject_args: Vec<&str> = subject_args.iter().map(String::as_str).collect();
    let run = dx_testing::run(&subject, &subject_args, &envs).expect("subject must execute");
    match mode.as_str() {
        "build" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "unexpected exit code for {}:\n{}",
                subject.display(),
                run.combined()
            );
            let stdout = run.stdout.replace("\r\n", "\n");
            assert_eq!(stdout.trim_end_matches('\n'), text, "unexpected output");
        }
        "pass" => assert_eq!(
            run.status.code(),
            Some(0),
            "passing subject failed:\n{}",
            run.combined()
        ),
        "coverage" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "coverage subject failed:\n{}",
                run.combined()
            );
            if native {
                let script = subject_env
                    .iter()
                    .find(|(name, _)| name == "CC_CODE_COVERAGE_SCRIPT")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                let collected = dx_testing::run(std::path::Path::new(&script), &[], &envs)
                    .expect("collector must execute");
                assert!(
                    collected.status.success(),
                    "collector failed:\n{}",
                    collected.combined()
                );
            }
            let mut report = std::fs::read_to_string(&out).unwrap_or_default();
            if let Ok(entries) = std::fs::read_dir(&raw) {
                for entry in entries.flatten() {
                    if entry.path().extension().is_some_and(|e| e == "dat") {
                        report.push('\n');
                        report.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
                    }
                }
            }
            if let Ok(entries) = std::fs::read_dir(&tmp) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with("coverage") && name.ends_with(".dat") {
                        report.push('\n');
                        report.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
                    }
                }
            }
            let report = report.replace('\\', "/");
            let source = format!("/{text}");
            assert!(
                sources(&report).iter().any(|name| name.ends_with(&source)),
                "no SF: for {text} in:\n{report}"
            );
            assert!(
                hits(&report, &source) > 0,
                "no covered lines in {text}:\n{report}"
            );
            let missed = misses(&report, &source);
            assert!(
                missed.is_empty(),
                "lines {missed:?} of {text} never ran:\n{report}"
            );
        }
        "lock" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "lock subject failed:\n{}",
                run.combined()
            );
            let stdout = run.stdout.replace("\r\n", "\n");
            assert_eq!(stdout.trim_end_matches('\n'), text, "unexpected output");
            let lock_input = |var: &str| {
                let key = std::env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
                dx_testing::rlocation(&key).expect("lock input must resolve from runfiles")
            };
            let depcheck = lock_input("DX_CONFORMANCE_DEPCHECK");
            let ecosystem = std::env::var("DX_CONFORMANCE_ECOSYSTEM").expect("ecosystem");
            let package = std::env::var("DX_CONFORMANCE_PACKAGE").expect("package");
            let manifest = lock_input("DX_CONFORMANCE_MANIFEST");
            let check = |lock: &std::path::Path| {
                let (manifest, lock) = (
                    manifest.to_string_lossy().into_owned(),
                    lock.to_string_lossy().into_owned(),
                );
                let args = [
                    "consistency",
                    "--ecosystem",
                    ecosystem.as_str(),
                    "--manifest",
                    manifest.as_str(),
                    "--lock",
                    lock.as_str(),
                ];
                dx_testing::run(&depcheck, &args, &[]).expect("depcheck must execute")
            };
            let fresh = check(&lock_input("DX_CONFORMANCE_LOCK"));
            assert!(
                fresh.status.success(),
                "lock check rejected the lockfile:\n{}",
                fresh.combined()
            );
            let stale = check(&lock_input("DX_CONFORMANCE_STALE"));
            assert!(
                !stale.status.success(),
                "lock check accepted the stale lockfile:\n{}",
                stale.combined()
            );
            let line = format!("missing lock entry for '{package}'");
            assert!(
                stale.combined().contains(&line),
                "missing {line:?} in:\n{}",
                stale.combined()
            );
        }
        "runfiles" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "runfiles directory run failed:\n{}",
                run.combined()
            );
            let stdout = run.stdout.replace("\r\n", "\n");
            assert_eq!(
                stdout.trim_end_matches('\n'),
                text,
                "unexpected output with a runfiles directory"
            );
            let manifest = manifest_file(&runfiles, std::path::Path::new(&tmp));
            let manifest = manifest.to_string_lossy().into_owned();
            let mut command = dx_testing::command_for(&subject, &envs).expect("command");
            command
                .env_remove("RUNFILES_DIR")
                .env_remove("TEST_SRCDIR")
                .env("RUNFILES_MANIFEST_ONLY", "1")
                .env("RUNFILES_MANIFEST_FILE", manifest.as_str())
                .args(&subject_args);
            let only = dx_testing::run_command(&mut command, std::time::Duration::from_secs(120))
                .expect("subject must execute");
            assert_eq!(
                only.status.code(),
                Some(0),
                "manifest-only run failed:\n{}",
                only.combined()
            );
            let stdout = only.stdout.replace("\r\n", "\n");
            assert_eq!(
                stdout.trim_end_matches('\n'),
                text,
                "unexpected output with a manifest only"
            );
        }
        "fail" => {
            assert!(
                !run.status.success(),
                "failing subject passed:\n{}",
                run.combined()
            );
            let line = failure_line(&pattern, &text);
            assert!(
                run.combined().contains(&line),
                "missing {line:?} in:\n{}",
                run.combined()
            );
        }
        other => panic!("unknown DX_CONFORMANCE_MODE {other}"),
    }
}

fn expected_image(platform: &str) -> (&'static str, &'static str) {
    let (os, cpu) = platform
        .split_once('_')
        .unwrap_or_else(|| panic!("unknown platform {platform}"));
    let format = match os {
        "linux" => "elf",
        "macos" => "macho",
        "windows" => "pe",
        "wasi" | "none" => "wasm",
        _ => panic!("unknown platform {platform}"),
    };
    let machine = match cpu {
        "arm64" => "aarch64",
        "x86_64" => "x86_64",
        "wasm32" => "wasm32",
        _ => panic!("unknown platform {platform}"),
    };
    (format, machine)
}

fn assert_image(key: &str, platform: &str) {
    let (format, machine) = expected_image(platform);
    let mut seen = Vec::new();
    for entry in key.split(' ').filter(|entry| !entry.is_empty()) {
        let path = dx_testing::rlocation(entry).expect("subject must resolve from runfiles");
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        match dx_testing::image_inspect(&bytes) {
            Ok(images) => {
                assert!(!images.is_empty(), "{entry}: no image");
                for image in images {
                    assert_eq!(
                        (image.format.name(), image.machine.as_str()),
                        (format, machine),
                        "{entry}: format and machine for {platform}"
                    );
                }
                return;
            }
            Err(error) => seen.push(format!("{entry}: {error}")),
        }
    }
    panic!("no binary image among the subject files: {seen:?}");
}

#[test]
fn expected_image_names_format_and_machine() {
    assert_eq!(expected_image("linux_arm64"), ("elf", "aarch64"));
    assert_eq!(expected_image("macos_arm64"), ("macho", "aarch64"));
    assert_eq!(expected_image("windows_x86_64"), ("pe", "x86_64"));
    assert_eq!(expected_image("wasi_wasm32"), ("wasm", "wasm32"));
    assert_eq!(expected_image("none_wasm32"), ("wasm", "wasm32"));
}

fn failure_line(pattern: &str, case: &str) -> String {
    pattern.replace("{case}", case)
}

#[test]
fn failure_line_substitutes_case() {
    assert_eq!(
        failure_line("--- FAIL: {case}", "TestFailingCase"),
        "--- FAIL: TestFailingCase"
    );
    assert_eq!(
        failure_line("test {case} ... FAILED", "failing_case"),
        "test failing_case ... FAILED"
    );
}

fn launcher_stem(name: &str) -> &str {
    match name.len().checked_sub(4) {
        Some(cut) if name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(".exe") => {
            &name[..cut]
        }
        _ => name,
    }
}

#[test]
fn launcher_stem_strips_exe() {
    assert_eq!(launcher_stem("hello.exe"), "hello");
    assert_eq!(launcher_stem("hello.EXE"), "hello");
    assert_eq!(launcher_stem("hello"), "hello");
    assert_eq!(launcher_stem("a.b/hello"), "a.b/hello");
}

fn collector_path(value: &str) -> String {
    let rest = if let Some(rest) = value.strip_prefix("external/") {
        rest.to_owned()
    } else if let Some(rest) = value.strip_prefix("bazel-out/") {
        match rest.splitn(3, '/').nth(2) {
            Some(rest) => format!("_main/{rest}"),
            None => return value.to_owned(),
        }
    } else {
        return value.to_owned();
    };
    dx_testing::rlocation(&rest)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| value.to_owned())
}

fn sources(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter_map(|line| line.strip_prefix("SF:"))
        .collect()
}

fn misses(report: &str, source: &str) -> Vec<String> {
    let mut inside = false;
    let mut missed = Vec::new();
    for line in report.lines() {
        if let Some(name) = line.strip_prefix("SF:") {
            inside = name.ends_with(source);
        } else if inside {
            if let Some(count) = line.strip_prefix("DA:") {
                let mut fields = count.split(',');
                let number = fields.next().unwrap_or_default();
                let hit = fields.next().and_then(|hit| hit.parse::<u64>().ok());
                if hit.unwrap_or(0) == 0 {
                    missed.push(number.to_owned());
                }
            }
        }
    }
    missed
}

fn hits(report: &str, source: &str) -> u64 {
    let mut inside = false;
    let mut total = 0;
    for line in report.lines() {
        if let Some(name) = line.strip_prefix("SF:") {
            inside = name.ends_with(source);
        } else if inside {
            if let Some(count) = line.strip_prefix("DA:") {
                total += count
                    .split(',')
                    .nth(1)
                    .and_then(|hit| hit.parse::<u64>().ok())
                    .unwrap_or(0);
            }
        }
    }
    total
}

fn manifest_file(runfiles: &str, tmp: &std::path::Path) -> std::path::PathBuf {
    if let Ok(manifest) = std::env::var("RUNFILES_MANIFEST_FILE") {
        return manifest.into();
    }
    let root = std::path::Path::new(runfiles);
    let mut lines = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("runfiles must be readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let (Ok(real), Ok(rel)) = (path.canonicalize(), path.strip_prefix(root)) {
                if rel == std::path::Path::new("MANIFEST") {
                    continue;
                }
                lines.push(format!("{} {}", rel.to_string_lossy(), real.to_string_lossy()));
            }
        }
    }
    lines.sort();
    let manifest = tmp.join("runfiles.manifest");
    std::fs::write(&manifest, lines.join("\n") + "\n").expect("manifest must be writable");
    manifest
}

fn with_suffix(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

fn stage(subject: &std::path::Path, key: &str, tmp: &std::path::Path) -> std::path::PathBuf {
    let beside = with_suffix(subject, ".runfiles");
    if beside.exists() || with_suffix(subject, ".runfiles_manifest").exists() {
        return subject.to_path_buf();
    }
    let dir = tmp.join("subject");
    std::fs::create_dir_all(&dir).expect("stage dir must be creatable");
    let staged = dir.join(subject.file_name().expect("subject must have a name"));
    let key = launcher_stem(key);
    let descriptor = dx_testing::rlocation(&format!("{key}.launch"))
        .or_else(|_| dx_testing::rlocation(&format!("{key}_upstream.launch")));
    let name = staged
        .file_name()
        .and_then(|name| name.to_str())
        .expect("staged subject must have a name")
        .to_owned();
    let stem = launcher_stem(&name).to_owned();
    if let Ok(descriptor) = descriptor {
        std::fs::copy(&descriptor, dir.join(format!("{stem}.launch")))
            .expect("descriptor must copy");
    }
    std::fs::copy(subject, &staged).expect("subject must copy");
    if let Some(Ok(siblings)) = subject.parent().map(std::fs::read_dir) {
        for sibling in siblings.flatten() {
            let file = sibling.file_name();
            let Some(file) = file.to_str() else { continue };
            if file.starts_with(&format!("{stem}.")) && !dir.join(file).exists() {
                std::fs::copy(sibling.path(), dir.join(file)).expect("sibling must copy");
            }
        }
    }
    #[cfg(unix)]
    if let Ok(tree) = std::env::var("RUNFILES_DIR") {
        for suffix in [".runfiles", "_upstream.runfiles"] {
            let mirror = with_suffix(&staged, suffix);
            std::fs::create_dir_all(&mirror).expect("runfiles mirror must be creatable");
            for entry in std::fs::read_dir(&tree)
                .expect("runfiles must be readable")
                .flatten()
            {
                std::os::unix::fs::symlink(entry.path(), mirror.join(entry.file_name()))
                    .expect("runfiles entry must link");
            }
        }
    }
    let manifest = manifest_source(
        std::env::var("RUNFILES_MANIFEST_FILE").ok(),
        &std::env::var("RUNFILES_DIR").unwrap_or_default(),
        &std::env::var("TEST_SRCDIR").unwrap_or_default(),
    );
    if let Some(manifest) = manifest {
        std::fs::copy(&manifest, with_suffix(&staged, ".runfiles_manifest"))
            .expect("manifest must copy");
    }
    staged
}

fn manifest_source(
    env_manifest: Option<String>,
    runfiles_dir: &str,
    srcdir: &str,
) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> =
        env_manifest.into_iter().map(Into::into).collect();
    for dir in [runfiles_dir, srcdir] {
        if !dir.is_empty() {
            candidates.push(std::path::Path::new(dir).join("MANIFEST"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

#[test]
fn manifest_source_falls_back_to_runfiles_dir() {
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set");
    let tree = std::path::Path::new(&tmp)
        .join("manifest_source")
        .join("tree");
    std::fs::create_dir_all(&tree).expect("tree must be creatable");
    std::fs::write(tree.join("MANIFEST"), "_main/a /x/a\n").expect("MANIFEST must write");
    let tree = tree.to_str().expect("tree must be utf-8");
    let missing = std::path::Path::new(&tmp)
        .join("manifest_source")
        .join("missing");
    let missing = missing.to_str().expect("missing must be utf-8");
    let expected = Some(std::path::Path::new(tree).join("MANIFEST"));
    assert_eq!(manifest_source(None, tree, missing), expected);
    assert_eq!(manifest_source(None, missing, tree), expected);
    assert_eq!(manifest_source(None, missing, missing), None);
    assert_eq!(manifest_source(None, "", ""), None);
}
