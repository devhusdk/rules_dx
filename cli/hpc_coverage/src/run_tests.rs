use crate::run::*;
use std::ffi::OsString;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hpc_coverage_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn finds_the_first_tix_file_and_the_mix_directories() {
    let dir = scratch("data");
    std::fs::create_dir_all(dir.join("mix/b-pkg")).unwrap();
    std::fs::create_dir_all(dir.join("mix/a-pkg")).unwrap();
    std::fs::write(dir.join("z.tix"), "").unwrap();
    std::fs::write(dir.join("greeter_test.tix"), "").unwrap();
    std::fs::write(dir.join("notes.txt"), "").unwrap();
    let data = hpc_data(&dir).unwrap();
    assert_eq!(data.tix, dir.join("greeter_test.tix"));
    assert_eq!(data.mix, [dir.join("mix/a-pkg"), dir.join("mix/b-pkg")]);
    assert_eq!(
        lcov_args(&dir.join("cov"), &data),
        [
            dir.join("cov/haskell.dat").into_os_string(),
            dir.join("greeter_test.tix").into_os_string(),
            dir.join("mix/a-pkg").into_os_string(),
            dir.join("mix/b-pkg").into_os_string(),
        ]
    );
}

#[test]
fn a_run_without_a_tix_file_has_no_data() {
    let dir = scratch("empty");
    std::fs::create_dir_all(dir.join("mix/a-pkg")).unwrap();
    assert_eq!(hpc_data(&dir), None);
    assert_eq!(hpc_data(&dir.join("absent")), None);
}

#[test]
fn a_run_needs_the_test_and_the_converter() {
    let none = |_: &str| -> Option<OsString> { None };
    assert!(run(&[], &none).is_err());
    assert!(run(&["test".into()], &none).is_err());
}

#[test]
fn a_missing_test_program_is_an_error_that_names_it() {
    let dir = scratch("missing");
    let test = dir.join("no_such_test").into_os_string();
    let error = run(&[test, "lcov".into()], &|_| None).unwrap_err();
    assert!(error.contains("no_such_test"), "{error}");
}
