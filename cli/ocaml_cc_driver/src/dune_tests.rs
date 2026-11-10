use crate::dune::*;
use std::fs;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parses_a_build_request() {
    let request = parse(&args(&[
        "build",
        "--workspace",
        "w",
        "--dune",
        "d",
        "--target",
        "a/b.exe",
        "--output",
        "o",
        "--jobs",
        "4",
        "--ocaml-bin",
        "bin",
    ]))
    .unwrap();
    assert_eq!(request.jobs, "4");
    assert_eq!(request.ocaml_bin, "bin");
    assert_eq!(
        request.step,
        Step::Build {
            workspace: "w".into(),
            dune: "d".into(),
            target: "a/b.exe".into(),
            output: "o".into()
        }
    );
}

#[test]
fn parses_library_archives_and_stubs() {
    let request = parse(&args(&[
        "library",
        "--workspace",
        "w",
        "--dune",
        "d",
        "--package",
        "p",
        "--stubs-name",
        "libp_stubs.a",
        "--stubs",
        "out/libp_stubs.a",
        "--tree",
        "t",
        "--archive",
        "p.cmxa=out/p.cmxa",
        "--archive",
        "p.a=out/p.a",
        "--jobs",
        "2",
        "--ocaml-bin",
        "bin",
    ]))
    .unwrap();
    match request.step {
        Step::Library {
            stubs, archives, ..
        } => {
            assert_eq!(stubs.as_deref(), Some("out/libp_stubs.a"));
            assert_eq!(
                archives,
                vec![
                    ("p.cmxa".to_owned(), "out/p.cmxa".to_owned()),
                    ("p.a".to_owned(), "out/p.a".to_owned())
                ]
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn rejects_bad_requests() {
    assert!(parse(&[]).is_err());
    assert!(
        parse(&args(&["nope"]))
            .unwrap_err()
            .contains("unknown mode")
    );
    assert!(
        parse(&args(&["bootstrap", "--source", "s"]))
            .unwrap_err()
            .contains("--output")
    );
    assert!(
        parse(&args(&["bootstrap", "x"]))
            .unwrap_err()
            .contains("unexpected")
    );
}

#[test]
fn drops_only_vendored_dirs_lines() {
    assert_eq!(without_vendored_dirs("(lang dune)\n"), None);
    assert_eq!(
        without_vendored_dirs("(a)\n(vendored_dirs x)\n (vendored_dirs y)\n(b)\n").as_deref(),
        Some("(a)\n (vendored_dirs y)\n(b)\n")
    );
}

#[test]
fn copy_tree_follows_links_and_writes_over_read_only_files() {
    let base = std::env::temp_dir().join(format!("dx_dune_test.{}", std::process::id()));
    let src = base.join("src");
    fs::create_dir_all(src.join("d")).unwrap();
    fs::write(src.join("d/f"), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(src.join("d/f"), src.join("link")).unwrap();
    let dst = base.join("dst");
    copy_tree(&src, &dst).unwrap();
    copy_tree(&src, &dst).unwrap();
    assert_eq!(fs::read_to_string(dst.join("d/f")).unwrap(), "x");
    #[cfg(unix)]
    assert!(fs::symlink_metadata(dst.join("link")).unwrap().is_file());
    fs::remove_dir_all(&base).unwrap();
}
