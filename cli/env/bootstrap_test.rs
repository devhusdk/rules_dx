use std::path::{Path, PathBuf};

fn env_bin() -> PathBuf {
    let rel = std::env::var("DX_ENV_BIN").expect("DX_ENV_BIN must name the env binary");
    dx_testing::resolve_runfiles(&rel)
}

fn staged_metadata() -> PathBuf {
    let rel = std::env::var("DX_STAGED_METADATA")
        .expect("DX_STAGED_METADATA must name the staged metadata");
    dx_testing::resolve_runfiles(&rel)
}

fn install(bin: &Path, root: &Path) -> String {
    let run = dx_testing::run(bin, &["--workspace", root.to_string_lossy().as_ref()], &[])
        .expect("env install must execute");
    assert!(
        run.status.success(),
        "fresh install failed\n{}",
        run.combined()
    );
    run.combined()
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create dst");
    let entries = std::fs::read_dir(src).expect("read src");
    for entry in entries {
        let entry = entry.expect("src entry");
        let file_type = entry.file_type().expect("file type");
        let dst_path = dst.join(entry.file_name());
        if file_type.is_symlink() {
            let target = std::fs::read_link(entry.path()).expect("read link");
            std::os::unix::fs::symlink(target, dst_path).expect("copy link");
        } else if file_type.is_dir() {
            copy_tree(&entry.path(), &dst_path);
        } else {
            std::fs::copy(entry.path(), dst_path).expect("copy file");
        }
    }
}

#[test]
fn env_bootstrap() {
    let bin = env_bin();
    let scratch = dx_testing::mkscratch("env-bootstrap-").expect("scratch");
    let root = scratch.join("work space");
    std::fs::create_dir_all(&root).expect("work space");

    let output = install(&bin, &root);
    assert!(
        output.contains("installed 3 tool(s)"),
        "unexpected fresh-install output: {output}"
    );

    for tool in ["doctor", "dx", "quality_markdown"] {
        assert!(
            is_symlink(&root.join(".dx/bin").join(tool)),
            "missing managed symlink: {tool}"
        );
    }
    assert!(
        root.join(".dx/bin/.rules_dx_managed").is_file()
            && !is_symlink(&root.join(".dx/bin/.rules_dx_managed")),
        "missing regular marker file"
    );
    assert!(
        !root.join(".dx/bin.next").exists() && !root.join(".dx/bin.prev").exists(),
        "staging leftovers after commit"
    );

    let doctor_bin = root.join(".dx/bin/doctor");
    let doctor = dx_testing::run(&doctor_bin, &[], &[]).expect("doctor must execute");
    assert!(
        doctor.status.success(),
        "doctor failed\n{}",
        doctor.combined()
    );
    assert!(
        doctor
            .combined()
            .contains("rules_dx managed environment tool: //env:doctor"),
        "unexpected doctor output: {}",
        doctor.combined()
    );

    let noop = install(&bin, &root);
    assert!(
        noop.contains("already current"),
        "second run was not a noop: {noop}"
    );

    let staged = staged_metadata();
    let staged_bin = staged.parent().expect("tree dir").join("bin");
    assert!(
        staged_bin.is_dir(),
        "staged bin not found under {}",
        staged_bin.display()
    );
    let alt = scratch.join("staged-alt");
    std::fs::create_dir_all(&alt).expect("alt");
    copy_tree(&staged_bin, &alt.join("bin"));
    let mut doc = dx_testing::read_json(&staged).expect("read staged metadata");
    let tools = doc
        .get_mut("tools")
        .and_then(|tools| tools.as_array_mut())
        .expect("tools array");
    tools.retain(|tool| {
        tool.get("bin_name").and_then(|name| name.as_str()) != Some("quality_markdown")
    });
    let alt_metadata = alt.join("metadata.json");
    dx_testing::write_json(&alt_metadata, &doc).expect("write alt metadata");
    std::fs::remove_file(alt.join("bin/quality_markdown")).expect("drop quality_markdown");

    let replace = dx_testing::run(
        &bin,
        &[
            "--workspace",
            root.to_string_lossy().as_ref(),
            "--staged-bin",
            alt.join("bin").to_string_lossy().as_ref(),
            "--metadata",
            alt_metadata.to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("env replace must execute");
    assert!(
        replace.status.success(),
        "replacement failed\n{}",
        replace.combined()
    );
    assert!(
        replace
            .combined()
            .contains("replaced managed tree with 2 tool(s)"),
        "unexpected replacement output: {}",
        replace.combined()
    );
    assert!(
        !root.join(".dx/bin/quality_markdown").exists(),
        "stale entry survived replacement"
    );
    assert!(
        is_symlink(&root.join(".dx/bin/doctor")),
        "surviving entry lost in replacement"
    );

    let restore = install(&bin, &root);
    assert!(
        restore.contains("replaced managed tree with 3 tool(s)"),
        "unexpected restore output: {restore}"
    );
    assert!(
        is_symlink(&root.join(".dx/bin/quality_markdown")),
        "restored entry missing"
    );

    let foreign = scratch.join("foreign");
    std::fs::create_dir_all(foreign.join(".dx/bin")).expect("foreign");
    std::fs::write(foreign.join(".dx/bin/stale_tool"), b"stale").expect("stale tool");
    let refused = dx_testing::run(
        &bin,
        &["--workspace", foreign.to_string_lossy().as_ref()],
        &[],
    )
    .expect("env refusal must execute");
    assert!(!refused.status.success(), "foreign tree was adopted");
    assert!(
        refused.combined().contains("refusing to touch unmanaged"),
        "missing unmanaged diagnostic: {}",
        refused.combined()
    );
    assert_eq!(
        std::fs::read_to_string(foreign.join(".dx/bin/stale_tool")).expect("read stale"),
        "stale",
        "foreign tree was modified"
    );
}
