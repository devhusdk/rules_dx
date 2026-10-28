use crate::opa::*;
use std::path::{Path, PathBuf};

const GOOD: &str = "version=1\nopa=_main/opa\nfile=_main/a.rego\tpolicy/a.rego\nfile=_main/d.json\tpolicy/d.json\n";

#[test]
fn parses_a_good_descriptor() {
    let parsed = parse(GOOD).unwrap();
    assert_eq!(parsed.opa, "_main/opa");
    assert_eq!(parsed.files.len(), 2);
    assert_eq!(parsed.files[1].1, "policy/d.json");
}

#[test]
fn rejects_bad_descriptors() {
    let cases = [
        "opa=x\nfile=a\tb\n",
        "version=2\nopa=x\nfile=a\tb\n",
        "version=1\nfile=a\tb\n",
        "version=1\nopa=x\nopa=y\nfile=a\tb\n",
        "version=1\nopa=x\n",
        "version=1\nopa=x\nbogus=1\nfile=a\tb\n",
        "version=1\nopa=x\nfile=a b\n",
        "version=1\nopa=x\nfile=a\tb\nfile=c\tb\n",
        "version=1\nopa=x\nfile=a\t/abs\n",
        "version=1\nopa=x\nfile=a\t../up\n",
        "version=1\nopa=x\nfile=a\tp//q\n",
        "version=1\nopa=x\nfile=a\tC:/q\n",
        "version=1\nopa=x\nfile=\tb\n",
    ];
    for case in cases {
        assert!(parse(case).is_err(), "accepted {case:?}");
    }
}

#[test]
fn builds_arguments() {
    assert_eq!(opa_args(None, &[]), ["test", "--fail-on-empty", "-v", "."]);
    assert_eq!(
        opa_args(Some("test_a"), &["--timeout=9s".to_owned()]),
        [
            "test",
            "--fail-on-empty",
            "-v",
            "--run",
            "test_a",
            "--timeout=9s",
            "."
        ]
    );
    assert_eq!(opa_args(Some(""), &[]).len(), 4);
}

fn fake_runfiles(dir: &Path) -> PathBuf {
    let exe = dir.join("tool");
    let src = dir.join("store");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("a.rego"), "package a\n").unwrap();
    std::fs::write(src.join("d.json"), "{}\n").unwrap();
    std::fs::write(
        dir.join("tool.runfiles_manifest"),
        format!(
            "_main/a.rego {}\n_main/d.json {}\n",
            src.join("a.rego").display(),
            src.join("d.json").display()
        ),
    )
    .unwrap();
    exe
}

#[test]
fn stages_from_a_manifest_only_runfiles() {
    let dir = tempfile_dir("stage");
    let exe = fake_runfiles(&dir);
    let resolver = resolver_for(&exe).unwrap();
    let descriptor = parse(
        GOOD.replace("opa=_main/opa\n", "opa=_main/a.rego\n")
            .as_str(),
    )
    .unwrap();
    let root = dir.join("scratch").join("rego_tree");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("stale.rego"), "package stale\n").unwrap();
    stage(&resolver, &descriptor, &root).unwrap();
    assert!(root.join("policy/a.rego").is_file());
    assert!(root.join("policy/d.json").is_file());
    assert!(!root.join("stale.rego").exists());
}

#[test]
fn names_the_missing_opa() {
    let dir = tempfile_dir("noopa");
    let exe = fake_runfiles(&dir);
    std::fs::write(dir.join("desc"), GOOD).unwrap();
    let mut manifest = std::fs::read_to_string(dir.join("tool.runfiles_manifest")).unwrap();
    manifest.push_str(&format!("_main/desc {}\n", dir.join("desc").display()));
    std::fs::write(dir.join("tool.runfiles_manifest"), manifest).unwrap();
    let error = run(&exe, "_main/desc", &[], None, &dir.join("scratch")).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("//config:tool_opa"), "{text}");
    assert!(text.contains("_main/opa"), "{text}");
}

fn tempfile_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rego_launcher_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
