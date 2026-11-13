use crate::run::*;
use std::ffi::OsString;

#[test]
fn finds_the_first_tix_file_and_the_mix_directories() {
    let scratch = dx_test_scratch::scratch("hpc_coverage_data_");
    let dir = scratch.path().to_path_buf();
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
    let scratch = dx_test_scratch::scratch("hpc_coverage_empty_");
    let dir = scratch.path().to_path_buf();
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
    let scratch = dx_test_scratch::scratch("hpc_coverage_missing_");
    let dir = scratch.path().to_path_buf();
    let test = dir.join("no_such_test").into_os_string();
    let error = run(&[test, "lcov".into()], &|_| None).unwrap_err();
    assert!(error.contains("no_such_test"), "{error}");
}
