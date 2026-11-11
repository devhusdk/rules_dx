#[test]
fn subject_matches_mode() {
    let mode = std::env::var("DX_CONFORMANCE_MODE").expect("DX_CONFORMANCE_MODE must be set");
    let key = std::env::var("DX_CONFORMANCE_SUBJECT").expect("DX_CONFORMANCE_SUBJECT must be set");
    let text = std::env::var("DX_CONFORMANCE_TEXT").unwrap_or_default();
    let subject = dx_testing::rlocation(&key).expect("subject must resolve from runfiles");
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set");
    let srcdir = std::env::var("TEST_SRCDIR").unwrap_or_default();
    let runfiles = std::env::var("RUNFILES_DIR").unwrap_or_else(|_| srcdir.clone());
    let mut envs = vec![
        ("PATH", ""),
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
            let mut report = std::fs::read_to_string(&out).unwrap_or_default();
            if let Ok(entries) = std::fs::read_dir(&raw) {
                for entry in entries.flatten() {
                    if entry.path().extension().is_some_and(|e| e == "dat") {
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
            assert!(hits(&report, &source) > 0, "no covered lines in {text}:\n{report}");
        }
        "fail" => {
            assert!(
                !run.status.success(),
                "failing subject passed:\n{}",
                run.combined()
            );
            let expected = format!("--- FAIL: {text}");
            assert!(
                run.combined().contains(&expected),
                "missing {expected:?} in:\n{}",
                run.combined()
            );
        }
        other => panic!("unknown DX_CONFORMANCE_MODE {other}"),
    }
}

fn sources(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter_map(|line| line.strip_prefix("SF:"))
        .collect()
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
