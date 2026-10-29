use crate::launch::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const GOOD: &str = "version=1\nprogram=_main/bin/php\narg=-n\narg=-d\narg=memory_limit=64M\npath=_main/x.phar\nprefixed=auto_prepend_file=\t_main/t.php/vendor/autoload.php\nenv_flag=--log-junit\tXML_OUTPUT_FILE\n";

#[test]
fn parses_a_good_descriptor() {
    let parsed = parse(GOOD).unwrap();
    assert_eq!(parsed.program, "_main/bin/php");
    assert_eq!(parsed.items.len(), 6);
    assert_eq!(
        parsed.items[4],
        Item::Prefixed(
            "auto_prepend_file=".to_owned(),
            "_main/t.php/vendor/autoload.php".to_owned()
        )
    );
    assert_eq!(
        parsed.items[5],
        Item::EnvFlag("--log-junit".to_owned(), "XML_OUTPUT_FILE".to_owned())
    );
}

#[test]
fn rejects_bad_descriptors() {
    let cases = [
        "program=x\n",
        "version=2\nprogram=x\n",
        "version=1\nversion=1\nprogram=x\n",
        "version=1\n",
        "version=1\nprogram=x\nprogram=y\n",
        "version=1\nprogram=\n",
        "version=1\nprogram=x\nbogus=1\n",
        "version=1\nprogram=x\npath=\n",
        "version=1\nprogram=x\nprefixed=a b\n",
        "version=1\nprogram=x\nprefixed=a\t\n",
        "version=1\nprogram=x\nprefixed=a\tb\tc\n",
        "version=1\nprogram=x\nenv_flag=\tVAR\n",
        "version=1\nprogram=x\nenv_flag=--f\n",
    ];
    for case in cases {
        assert!(parse(case).is_err(), "accepted {case:?}");
    }
}

#[test]
fn keeps_literal_arguments_with_spaces_and_equals() {
    let parsed = parse("version=1\nprogram=x\narg=-d a=b c='d'\n").unwrap();
    assert_eq!(parsed.items, [Item::Arg("-d a=b c='d'".to_owned())]);
}

#[test]
fn names_the_descriptor_beside_the_executable() {
    assert_eq!(
        descriptor_path(Path::new("out/app.exe")),
        PathBuf::from("out/app.launch")
    );
    assert_eq!(
        descriptor_path(Path::new("out/app.test.exe")),
        PathBuf::from("out/app.test.launch")
    );
}

#[test]
fn falls_back_to_the_upstream_descriptor() {
    let dir = scratch("upstream");
    std::fs::write(dir.join("app_upstream.launch"), "").unwrap();
    assert_eq!(
        descriptor_path(&dir.join("app.exe")),
        dir.join("app_upstream.launch")
    );
    std::fs::write(dir.join("app.launch"), "").unwrap();
    assert_eq!(
        descriptor_path(&dir.join("app.exe")),
        dir.join("app.launch")
    );
    assert_eq!(
        descriptor_path(&dir.join("other.exe")),
        dir.join("other.launch")
    );
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exec_launcher_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn expected_args(root: &Path, with_junit: bool) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-n".into(), "-d".into(), "memory_limit=64M".into()];
    args.push(root.join("x.phar").into_os_string());
    let mut prefixed = OsString::from("auto_prepend_file=");
    prefixed.push(root.join("t.php/vendor/autoload.php"));
    args.push(prefixed);
    if with_junit {
        args.push("--log-junit".into());
        args.push("/out/junit.xml".into());
    }
    args
}

#[test]
fn plans_from_a_runfiles_tree() {
    let dir = scratch("tree");
    let tree = dir.join("app.runfiles");
    let main = tree.join("_main");
    std::fs::create_dir_all(main.join("bin")).unwrap();
    std::fs::create_dir_all(main.join("t.php/vendor")).unwrap();
    std::fs::write(main.join("bin/php"), "").unwrap();
    std::fs::write(main.join("x.phar"), "").unwrap();
    std::fs::write(main.join("t.php/vendor/autoload.php"), "").unwrap();
    let resolver = resolver_for(&dir.join("app.exe")).unwrap();
    assert_eq!(runfiles_env(&resolver), ("RUNFILES_DIR", tree.clone()));
    let descriptor = parse(GOOD).unwrap();

    let none = plan(&resolver, &descriptor, &|_| None).unwrap();
    assert_eq!(none.program, main.join("bin/php"));
    assert_eq!(none.args, expected_args(&main, false));

    let empty = plan(&resolver, &descriptor, &|_| Some(String::new())).unwrap();
    assert_eq!(empty.args, expected_args(&main, false));

    let junit = plan(&resolver, &descriptor, &|name| {
        (name == "XML_OUTPUT_FILE").then(|| "/out/junit.xml".to_owned())
    })
    .unwrap();
    assert_eq!(junit.args, expected_args(&main, true));
}

#[test]
fn plans_from_a_manifest_with_a_tree_artifact() {
    let dir = scratch("manifest");
    let store = dir.join("store");
    std::fs::create_dir_all(store.join("t.php/vendor")).unwrap();
    std::fs::write(store.join("php"), "").unwrap();
    std::fs::write(store.join("x.phar"), "").unwrap();
    std::fs::write(store.join("t.php/vendor/autoload.php"), "").unwrap();
    std::fs::write(
        dir.join("app.exe.runfiles_manifest"),
        format!(
            "_main/bin/php {}\n_main/x.phar {}\n_main/t.php {}\n",
            store.join("php").display(),
            store.join("x.phar").display(),
            store.join("t.php").display()
        ),
    )
    .unwrap();
    let resolver = resolver_for(&dir.join("app.exe")).unwrap();
    assert_eq!(
        runfiles_env(&resolver),
        (
            "RUNFILES_MANIFEST_FILE",
            dir.join("app.exe.runfiles_manifest")
        )
    );
    let planned = plan(&resolver, &parse(GOOD).unwrap(), &|_| None).unwrap();
    assert_eq!(planned.program, store.join("php"));
    assert_eq!(planned.args, expected_args(&store, false));
}

#[test]
fn names_a_missing_runfile() {
    let dir = scratch("missing");
    let tree = dir.join("app.runfiles");
    std::fs::create_dir_all(tree.join("_main")).unwrap();
    let resolver = resolver_for(&dir.join("app.exe")).unwrap();
    let error = plan(&resolver, &parse(GOOD).unwrap(), &|_| None).unwrap_err();
    assert!(error.to_string().contains("_main/bin/php"), "{error}");
}

#[cfg(unix)]
#[test]
fn runs_the_program_and_passes_the_exit_code() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("run");
    let main = dir.join("app.runfiles/_main");
    std::fs::create_dir_all(&main).unwrap();
    let script = main.join("prog");
    std::fs::write(&script, "#!/bin/sh\nexit $(( $# + 4 ))\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        dir.join("app.launch"),
        "version=1\nprogram=_main/prog\narg=one\n",
    )
    .unwrap();
    let code = run(&dir.join("app.exe"), &["two".to_owned()]).unwrap();
    assert_eq!(code, 6);
}
