use crate::run::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[test]
fn a_mix_file_belongs_to_its_first_hpc_ancestor() {
    assert_eq!(
        hpc_dir(Path::new("/r/pkg/lib_.hpc/lib-0.1/Greet.mix")),
        Path::new("/r/pkg/lib_.hpc")
    );
    assert_eq!(
        hpc_dir(Path::new("/r/pkg/test_.hpc/Main.mix")),
        Path::new("/r/pkg/test_.hpc")
    );
    assert_eq!(
        hpc_dir(Path::new("/r/other/Main.mix")),
        Path::new("/r/other")
    );
}

#[test]
fn the_hpc_directories_are_distinct_and_sorted() {
    let mix: Vec<OsString> = ["/r/b_.hpc/B.mix", "/r/a_.hpc/p/A.mix", "/r/b_.hpc/C.mix"]
        .into_iter()
        .map(OsString::from)
        .collect();
    assert_eq!(
        hpc_dirs(&mix),
        [PathBuf::from("/r/a_.hpc"), PathBuf::from("/r/b_.hpc")]
    );
}

#[test]
fn the_tix_file_is_named_after_the_test_binary() {
    assert_eq!(
        tix_path(Path::new("/out"), Path::new("/r/pass_test.exe")),
        Path::new("/out/pass_test.tix")
    );
    assert_eq!(
        tix_path(Path::new("/out"), Path::new("/r/pass_test")),
        Path::new("/out/pass_test.tix")
    );
}

#[test]
fn the_converter_gets_the_lcov_file_the_tix_file_and_the_hpc_directories() {
    assert_eq!(
        lcov_args(
            Path::new("/cov"),
            Path::new("/out/t.tix"),
            &[PathBuf::from("/r/a_.hpc")]
        ),
        [
            Path::new("/cov").join("haskell.dat").into_os_string(),
            OsString::from("/out/t.tix"),
            OsString::from("/r/a_.hpc"),
        ]
    );
}

#[test]
fn a_run_needs_the_test_the_converter_and_the_separator() {
    let none = |_: &str| -> Option<OsString> { None };
    assert!(run(&[], &none).is_err());
    assert!(run(&["test".into()], &none).is_err());
    assert!(run(&["test".into(), "lcov".into()], &none).is_err());
}

#[test]
fn a_missing_test_program_is_an_error_that_names_it() {
    let scratch = dx_test_scratch::scratch("hpc_coverage_missing_").unwrap();
    let dir = scratch.path().to_path_buf();
    let test = dir.join("no_such_test").into_os_string();
    let error = run(&[test, "lcov".into(), "--".into()], &|_| None).unwrap_err();
    assert!(error.contains("no_such_test"), "{error}");
}

#[test]
fn a_covered_run_without_a_tix_file_fails() {
    let scratch = dx_test_scratch::scratch("hpc_coverage_no_tix_").unwrap();
    let dir = scratch.path().to_path_buf();
    let test = std::env::current_exe().unwrap().into_os_string();
    let outputs = dir.clone().into_os_string();
    let env = move |name: &str| match name {
        "COVERAGE_DIR" => Some(OsString::from("cov")),
        "TEST_UNDECLARED_OUTPUTS_DIR" => Some(outputs.clone()),
        _ => None,
    };
    let args = [
        test,
        "lcov".into(),
        "--".into(),
        "--exact".into(),
        "run_tests::the_tix_file_is_named_after_the_test_binary".into(),
    ];
    assert_eq!(run(&args, &env), Ok(1));
}
