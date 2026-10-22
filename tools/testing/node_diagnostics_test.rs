use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn tsc(source: &str, extra: &[&str]) -> dx_testing::Run {
    let path = text(file(source));
    let mut args = vec![
        "--noEmit",
        "--strict",
        "--target",
        "ES2022",
        "--module",
        "ESNext",
        "--moduleResolution",
        "Bundler",
    ];
    args.extend_from_slice(extra);
    args.push(path.as_str());
    dx_testing::run(&file("DX_TSC"), &args, &[]).expect("tsc runs")
}

fn biome(command: &str, source: &str, config: &str) -> dx_testing::Run {
    let path = text(file(source));
    let config_arg = format!("--config-path={}", text(file(config)));
    dx_testing::run(
        &file("DX_BIOME"),
        &[command, config_arg.as_str(), path.as_str()],
        &[],
    )
    .expect("biome runs")
}

#[test]
fn clean_source_typechecks_lints_and_formats() {
    for extra in [&[][..], &["--declaration", "--isolatedDeclarations"][..]] {
        let built = tsc("DX_CLEAN", extra);
        assert!(built.status.success(), "{extra:?}: {}", built.combined());
    }
    let linted = biome("lint", "DX_CLEAN", "DX_CONFIG");
    assert!(linted.status.success(), "{}", linted.combined());
    let formatted = biome("format", "DX_CLEAN", "DX_CONFIG");
    assert!(formatted.status.success(), "{}", formatted.combined());
}

#[test]
fn syntax_error_is_rejected() {
    let run = tsc("DX_SYNTAX", &[]);
    assert!(!run.status.success());
    assert!(run.combined().contains("TS1109"), "{}", run.combined());
    let formatted = biome("format", "DX_SYNTAX", "DX_CONFIG");
    assert!(!formatted.status.success());
}

#[test]
fn type_error_is_rejected() {
    let run = tsc("DX_TYPE", &[]);
    assert!(!run.status.success());
    assert!(run.combined().contains("TS2322"), "{}", run.combined());
}

#[test]
fn isolated_declarations_mode_rejects_an_inferred_export() {
    let plain = tsc("DX_ISOLATED", &[]);
    assert!(plain.status.success(), "{}", plain.combined());
    let strict = tsc("DX_ISOLATED", &["--declaration", "--isolatedDeclarations"]);
    assert!(!strict.status.success());
    assert!(
        strict.combined().contains("TS9007"),
        "{}",
        strict.combined()
    );
}

#[test]
fn lint_finding_is_an_error_in_both_languages() {
    for var in ["DX_LINT_TS", "DX_LINT_JS"] {
        let run = biome("lint", var, "DX_CONFIG");
        assert!(!run.status.success(), "{var}");
        assert!(
            run.combined().contains("noDoubleEquals"),
            "{var}: {}",
            run.combined()
        );
    }
}

#[test]
fn unformatted_source_is_rejected() {
    let run = biome("format", "DX_FORMAT", "DX_CONFIG");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("Formatter would have printed"),
        "{}",
        run.combined()
    );
}

#[test]
fn the_consumer_config_changes_the_verdict() {
    let repo_cfg = biome("format", "DX_CLEAN", "DX_CONFIG");
    assert!(repo_cfg.status.success(), "{}", repo_cfg.combined());
    let other = biome("format", "DX_CLEAN", "DX_OTHER_CONFIG");
    assert!(!other.status.success());
    assert!(
        other.combined().contains("Formatter would have printed"),
        "{}",
        other.combined()
    );
}
