#[test]
fn subject_matches_mode() {
    let mode = std::env::var("DX_CONFORMANCE_MODE").expect("DX_CONFORMANCE_MODE must be set");
    let key = std::env::var("DX_CONFORMANCE_SUBJECT").expect("DX_CONFORMANCE_SUBJECT must be set");
    let text = std::env::var("DX_CONFORMANCE_TEXT").unwrap_or_default();
    let pattern = std::env::var("DX_CONFORMANCE_PATTERN").unwrap_or_else(|_| "{case}".to_owned());
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
    let run = dx_testing::run(&subject, &[], &envs).expect("subject must execute");
    match mode.as_str() {
        "build" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "unexpected exit code for {}",
                subject.display()
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
    let descriptor = dx_testing::rlocation(&format!("{key}.launch"))
        .or_else(|_| dx_testing::rlocation(&format!("{key}_upstream.launch")));
    if let Ok(descriptor) = descriptor {
        std::fs::copy(&descriptor, with_suffix(&staged, ".launch")).expect("descriptor must copy");
    }
    std::fs::copy(subject, &staged).expect("subject must copy");
    let name = staged
        .file_name()
        .and_then(|name| name.to_str())
        .expect("staged subject must have a name")
        .to_owned();
    if let Some(Ok(siblings)) = subject.parent().map(std::fs::read_dir) {
        for sibling in siblings.flatten() {
            let file = sibling.file_name();
            let Some(file) = file.to_str() else { continue };
            if file.starts_with(&format!("{name}.")) && !dir.join(file).exists() {
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
    if let Ok(manifest) = std::env::var("RUNFILES_MANIFEST_FILE") {
        std::fs::copy(&manifest, with_suffix(&staged, ".runfiles_manifest"))
            .expect("manifest must copy");
    }
    staged
}
