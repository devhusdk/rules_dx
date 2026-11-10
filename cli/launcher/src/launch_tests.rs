use crate::launch::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const GOOD: &str = "program=_main/bin/php\narg=-n\narg=-d\narg=memory_limit=64M\npath=_main/x.phar\nprefixed=auto_prepend_file=\t_main/t.php/vendor/autoload.php\nenv_flag=--log-junit\tXML_OUTPUT_FILE\n";

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
        "version=1\nprogram=x\n",
        "",
        "program=x\nprogram=y\n",
        "program=\n",
        "program=x\nbogus=1\n",
        "program=x\npath=\n",
        "program=x\nprefixed=a b\n",
        "program=x\nprefixed=a\t\n",
        "program=x\nprefixed=a\tb\tc\n",
        "program=x\nenv_flag=\tVAR\n",
        "program=x\nenv_flag=--f\n",
        "program=x\nunset=\n",
        "program=x\nunset=A=B\n",
        "program=x\nenv=A\n",
        "program=x\nenv=\tv\n",
        "program=x\nenv=A=B\tv\n",
    ];
    for case in cases {
        assert!(parse(case).is_err(), "accepted {case:?}");
    }
}

#[test]
fn keeps_literal_arguments_with_spaces_and_equals() {
    let parsed = parse("program=x\narg=-d a=b c='d'\n").unwrap();
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

#[test]
fn finds_the_exe_runfiles_for_an_extensionless_argv0() {
    let dir = scratch("extensionless");
    let tree = dir.join("app.exe.runfiles");
    std::fs::create_dir_all(tree.join("_main")).unwrap();
    std::fs::write(dir.join("app.launch"), "").unwrap();
    assert_eq!(descriptor_path(&dir.join("app")), dir.join("app.launch"));
    let resolver = resolver_for(&dir.join("app")).unwrap();
    assert_eq!(runfiles_env(&resolver), ("RUNFILES_DIR", tree.clone()));
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

    let empty = plan(&resolver, &descriptor, &|_| Some(OsString::new())).unwrap();
    assert_eq!(empty.args, expected_args(&main, false));

    let junit = plan(&resolver, &descriptor, &|name| {
        (name == "XML_OUTPUT_FILE").then(|| OsString::from("/out/junit.xml"))
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
    let dir = scratch("run");
    let main = dir.join("app.runfiles/_main");
    std::fs::create_dir_all(&main).unwrap();
    let script = main.join("prog");
    dx_testing::executable_script(&script, "#!/bin/sh\nexit $(( $# + 4 ))\n");
    std::fs::write(dir.join("app.launch"), "program=_main/prog\narg=one\n").unwrap();
    let code = run(&dir.join("app.exe"), &[OsString::from("two")]).unwrap();
    assert_eq!(code, 6);
}

#[test]
fn parses_unset_lines_apart_from_items() {
    let parsed = parse("program=x\nunset=QT_PLUGIN_PATH\n").unwrap();
    assert_eq!(parsed.unset, ["QT_PLUGIN_PATH"]);
    assert!(parsed.items.is_empty());
}

#[test]
fn command_removes_unset_names_and_sets_runfiles() {
    let plan = Plan {
        program: PathBuf::from("prog"),
        args: Vec::new(),
    };
    let unset = ["QT_PLUGIN_PATH".to_owned()];
    let built = command(
        &plan,
        &unset,
        &[],
        ("RUNFILES_DIR", PathBuf::from("/r")),
        &[],
    );
    let envs: Vec<_> = built.get_envs().collect();
    assert!(envs.contains(&(
        std::ffi::OsStr::new("QT_PLUGIN_PATH"),
        None::<&std::ffi::OsStr>
    )));
    assert!(envs.contains(&(
        std::ffi::OsStr::new("RUNFILES_DIR"),
        Some(std::ffi::OsStr::new("/r"))
    )));
}

#[cfg(unix)]
#[test]
fn run_scrubs_unset_variables() {
    let dir = scratch("unset");
    let main = dir.join("app.runfiles/_main");
    std::fs::create_dir_all(&main).unwrap();
    let script = main.join("prog");
    dx_testing::executable_script(
        &script,
        "#!/bin/sh\nexit ${EXEC_LAUNCHER_UNSET_PROBE:+9}0\n",
    );
    std::fs::write(
        dir.join("app.launch"),
        "program=_main/prog\nunset=EXEC_LAUNCHER_UNSET_PROBE\n",
    )
    .unwrap();
    // Only this test uses this name, so setting it cannot disturb the others.
    unsafe { std::env::set_var("EXEC_LAUNCHER_UNSET_PROBE", "1") };
    let code = run(&dir.join("app.exe"), &[]).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn parses_env_lines_apart_from_items() {
    let parsed = parse("program=x\nenv=MIX_HOME\tC:/a b\n").unwrap();
    assert_eq!(parsed.env, [("MIX_HOME".to_owned(), "C:/a b".to_owned())]);
    assert!(parsed.items.is_empty());
}

#[test]
fn command_sets_descriptor_env() {
    let plan = Plan {
        program: PathBuf::from("prog"),
        args: Vec::new(),
    };
    let env = [("MIX_HOME".to_owned(), "/h".to_owned())];
    let built = command(&plan, &[], &env, ("RUNFILES_DIR", PathBuf::from("/r")), &[]);
    let envs: Vec<_> = built.get_envs().collect();
    assert!(envs.contains(&(
        std::ffi::OsStr::new("MIX_HOME"),
        Some(std::ffi::OsStr::new("/h"))
    )));
}

#[cfg(unix)]
#[test]
fn command_forwards_non_utf8_arguments() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let plan = Plan {
        program: PathBuf::from("prog"),
        args: Vec::new(),
    };
    let raw = OsString::from_vec(vec![0x66, 0xff, 0x6f]);
    let built = command(
        &plan,
        &[],
        &[],
        ("RUNFILES_DIR", PathBuf::from("/r")),
        &[raw],
    );
    let args: Vec<_> = built.get_args().collect();
    assert_eq!(args[0].as_bytes(), &[0x66, 0xff, 0x6f]);
}
