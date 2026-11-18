use super::*;

static SHFMT_FALLBACK_SPAWNS: SpawnCount = SpawnCount::new();

fn partial_shfmt(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    SHFMT_FALLBACK_SPAWNS.bump();
    let files = file_args(argv, "sh");
    let broken = files.iter().any(|file| {
        std::fs::read_to_string(file)
            .expect("file is materialized")
            .contains("BROKEN")
    });
    for file in &files {
        let bytes = std::fs::read(file).expect("file is materialized");
        let written = if broken {
            b"PARTIAL\n".to_vec()
        } else {
            trimmed(&bytes)
        };
        std::fs::write(file, written).expect("fix writes back");
    }
    ok_output(i32::from(broken), String::new())
}

fn sh_stage(count: usize, pad: usize) -> (crate::StageSpec, BTreeMap<String, String>) {
    let mut current = BTreeMap::new();
    for index in 0..count {
        current.insert(
            format!("bin/s{index:04}{}.sh", "s".repeat(pad)),
            "echo  \n".to_owned(),
        );
    }
    let refs: Vec<&str> = current.keys().map(String::as_str).collect();
    (stage("shfmt", &["shell"], &refs), current)
}

#[test]
fn batch_fix_failure_falls_back_to_single_files() {
    SHFMT_FALLBACK_SPAWNS.reset();
    let (spec, mut current) = sh_stage(4, 0);
    let broken = spec.source_paths[2].clone();
    current.insert(broken.clone(), "BROKEN  \n".to_owned());
    let backend = backend_for("shfmt", plain_tool(), partial_shfmt);
    let known = BTreeSet::new();
    let mut session = real_fix::FixSession::new(&backend, "format", &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("fallback fixes");
    session.close().expect("session closes");
    assert_eq!(SHFMT_FALLBACK_SPAWNS.get(), 5);
    for (path, text) in &fixed {
        let want = if *path == broken {
            "BROKEN  \n"
        } else {
            "echo\n"
        };
        assert_eq!(text, want, "{path}");
    }
    assert_eq!(fixed.len(), 4);
}

static SHFMT_BUDGET_SPAWNS: SpawnCount = SpawnCount::new();

fn budget_shfmt(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    SHFMT_BUDGET_SPAWNS.bump();
    assert_within_budget(argv);
    partial_shfmt(argv, cwd, env)
}

#[test]
fn large_fix_selection_splits_within_argv_budget() {
    SHFMT_BUDGET_SPAWNS.reset();
    let (spec, current) = sh_stage(300, 120);
    let backend = backend_for("shfmt", plain_tool(), budget_shfmt);
    let known = BTreeSet::new();
    let mut session = real_fix::FixSession::new(&backend, "format", &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("batches fix");
    session.close().expect("session closes");
    let spawns = SHFMT_BUDGET_SPAWNS.get();
    assert!(spawns > 1 && spawns < 300, "{spawns} spawns");
    assert_eq!(fixed.len(), 300);
    assert!(fixed.values().all(|text| text == "echo\n"));
}
