//! Materializes a hermetic consumer on the nested Bazel harness and builds, tests and covers it
//! offline from its cache set.

use dx_testing::Run;

fn ok(run: &Run, what: &str) {
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

fn with_platform(manifest: &hermetic::Manifest, args: &[String]) -> Vec<String> {
    let mut argv = args.to_vec();
    if !manifest.platform.is_empty() {
        argv.push(format!("--platforms={}", manifest.platform));
    }
    argv
}

/// The `PATH` of the nested Bazel: the coreutils applets, or none on Windows.
fn applets_path() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let applets = dx_testing::env_files("DX_COREUTILS");
    let dir = applets
        .first()
        .and_then(|file| file.parent())
        .expect("DX_COREUTILS names the applets");
    let dir = std::path::absolute(dir).expect("applet directory");
    Some(dir.to_string_lossy().into_owned())
}

fn bazel(case: &dx_nested::Case, manifest: &hermetic::Manifest, args: &[String]) -> Run {
    let argv = with_platform(manifest, args);
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    match applets_path() {
        Some(path) => case.bazel_with(&argv, &[("PATH", path.as_str())]),
        None => case.bazel(&argv),
    }
}

#[test]
fn consumer_builds_tests_and_covers_offline() {
    let rel =
        std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST names the manifest");
    let set = std::env::var("DX_NESTED_CACHE_SET").expect("DX_NESTED_CACHE_SET names the set");
    let text = dx_testing::read_text(&dx_testing::resolve_runfiles(&rel));
    let base = std::env::current_dir().expect("runfiles cwd");
    let manifest = hermetic::parse_manifest(&text, &base).expect("manifest parses");
    let source = hermetic::source_tree(&manifest).unwrap_or_else(|error| panic!("{error}"));
    let case = dx_nested::case(&set);
    hermetic::materialize(&manifest, case.root(), &source, "cc")
        .unwrap_or_else(|error| panic!("{error}"));
    case.use_lock(&set);

    let mut targets = manifest.patterns.clone();
    if !manifest.coverage.is_empty() {
        targets.push(format!("@{}//dx:generate_check", manifest.repo_name));
    }
    let mut test = vec!["test".to_owned(), "--test_output=errors".to_owned()];
    test.extend(targets);
    ok(&bazel(&case, &manifest, &test), "bazel test");

    for (label, marker) in &manifest.failures {
        let run = bazel(&case, &manifest, &["build".to_owned(), label.clone()]);
        let log = run.combined();
        assert_ne!(run.status.code(), Some(0), "{label} must fail to build");
        assert!(
            log.contains(marker.as_str()),
            "{label} fails without {marker:?}: {log}"
        );
    }

    if !manifest.coverage.is_empty() {
        let mut coverage = vec![
            "coverage".to_owned(),
            "--combined_report=lcov".to_owned(),
            "--test_output=errors".to_owned(),
        ];
        coverage.extend(manifest.coverage.iter().cloned());
        ok(&bazel(&case, &manifest, &coverage), "bazel coverage");
        let report =
            dx_testing::read_text(&case.root().join("bazel-out/_coverage/_coverage_report.dat"));
        let sources = hermetic::coverage_sources(&report).unwrap_or_else(|error| panic!("{error}"));
        hermetic::check_coverage(&sources, &manifest).unwrap_or_else(|error| panic!("{error}"));
    }
}
