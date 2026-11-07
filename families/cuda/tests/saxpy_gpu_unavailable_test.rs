use dx_testing::fatbin;

fn binary() -> std::path::PathBuf {
    dx_testing::resolve_runfiles(&std::env::var("DX_GPU_TEST").unwrap())
}

#[test]
fn a_host_without_a_visible_gpu_fails_as_unavailable() {
    let outputs = dx_testing::mkscratch("gpu-outputs").unwrap();
    let outputs_dir = outputs.to_str().unwrap();
    let run = dx_testing::run(
        &binary(),
        &[],
        &[
            ("CUDA_VISIBLE_DEVICES", ""),
            ("TEST_UNDECLARED_OUTPUTS_DIR", outputs_dir),
        ],
    )
    .unwrap();
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let first = run.stderr.lines().next().unwrap_or_default();
    assert!(first.starts_with("unavailable: "), "{}", run.combined());
    let facts = dx_testing::read_json(&outputs.join("gpu_facts.json")).unwrap();
    assert_eq!(facts["schema"], "dx.cuda.gpu_facts.v1");
    assert_eq!(facts["status"], "unavailable");
    assert_eq!(facts["reason"], first.trim_start_matches("unavailable: "));
    assert_eq!(facts["declared"]["min_compute_capability"], "8.9");
    assert_eq!(facts["declared"]["min_driver"], "13.0");
    assert_eq!(facts["declared"]["runtime"], "13.4");
    assert_eq!(facts["runtime"], "13.4");
    assert!(facts["device"].is_null(), "{facts}");
    assert_eq!(facts["checks"], dx_testing::serde_json::json!([]));
    let logged = run
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("dx-gpu-facts: "))
        .unwrap_or_else(|| panic!("no dx-gpu-facts line: {}", run.combined()));
    let logged: dx_testing::serde_json::Value = dx_testing::serde_json::from_str(logged).unwrap();
    assert_eq!(logged, facts);
}

#[test]
fn missing_declarations_fail_before_any_cuda_call() {
    let outputs = dx_testing::mkscratch("gpu-declared").unwrap();
    let run = dx_testing::run(
        &binary(),
        &[],
        &[
            ("DX_GPU_MIN_CC", ""),
            ("TEST_UNDECLARED_OUTPUTS_DIR", outputs.to_str().unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(
        run.stderr.lines().next(),
        Some("fail: DX_GPU_MIN_CC is missing or not X.Y"),
        "{}",
        run.combined()
    );
    let facts = dx_testing::read_json(&outputs.join("gpu_facts.json")).unwrap();
    assert_eq!(facts["status"], "fail");
    assert!(facts["driver"].is_null(), "{facts}");
}

#[test]
fn the_probe_links_the_static_runtime() {
    let bytes = std::fs::read(binary()).unwrap();
    let needed = fatbin::needed_libraries(&bytes).unwrap();
    assert!(!needed.is_empty(), "the binary links libc dynamically");
    assert!(
        needed
            .iter()
            .all(|lib| !lib.starts_with("libcudart") && !lib.starts_with("libcuda.")),
        "the probe must link the static runtime and load the driver at run time: {needed:?}"
    );
}
