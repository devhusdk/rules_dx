use std::path::Path;

fn stage(dir: &Path) {
    for (env, name) in [
        ("DX_TSCONFIG_BASE", "tsconfig.json"),
        ("DX_TSCONFIG_GOOD", "tsconfig_good.json"),
        ("DX_TSCONFIG_CLEAN", "tsconfig_clean.json"),
        ("DX_SRC_GOOD", "good.ts"),
        ("DX_SRC_CLEAN", "clean.ts"),
        ("DX_SRC_BAD", "bad.ts"),
    ] {
        let rel = std::env::var(env).unwrap_or_else(|_| panic!("{env} must be set"));
        let from = dx_testing::resolve_runfiles(&rel);
        std::fs::copy(&from, dir.join(name))
            .unwrap_or_else(|error| panic!("cannot stage {}: {error}", from.display()));
    }
}

fn list_files(dir: &Path, config: &str) -> (bool, Vec<String>, String) {
    let rel = std::env::var("DX_TSC").expect("DX_TSC must name tsc");
    let tsc = dx_testing::resolve_runfiles(&rel);
    let project = dir.join(config);
    let run = dx_testing::run(
        &tsc,
        &[
            "--listFilesOnly",
            "--noEmit",
            "-p",
            project.to_str().unwrap(),
        ],
        &[],
    )
    .expect("tsc must execute");
    let root = dir.to_string_lossy().replace('\\', "/");
    let files = run
        .stdout
        .lines()
        .map(|line| line.trim().replace('\\', "/"))
        .filter(|line| line.starts_with(&root))
        .map(|line| line[root.len()..].trim_start_matches('/').to_owned())
        .collect();
    (run.status.success(), files, run.combined())
}

#[test]
fn scoped_tsconfig_lists_only_its_own_sources() {
    let dir = dx_testing::mkscratch("scoped-boundary").expect("scratch dir");
    stage(&dir);

    let (_, inherited, _) = list_files(&dir, "tsconfig.json");
    assert!(
        inherited.iter().any(|f| f == "bad.ts"),
        "control: inherited include must see bad.ts, saw {inherited:?}"
    );

    let (ok, good, output) = list_files(&dir, "tsconfig_good.json");
    assert!(ok, "good must typecheck beside bad.ts:\n{output}");
    assert_eq!(good, vec!["good.ts"], "good root files");

    let (ok, clean, output) = list_files(&dir, "tsconfig_clean.json");
    assert!(ok, "clean must typecheck beside bad.ts:\n{output}");
    assert_eq!(clean, vec!["clean.ts"], "clean root files");
}
