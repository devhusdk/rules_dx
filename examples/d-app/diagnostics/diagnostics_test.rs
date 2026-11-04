use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn ldc(args: &[&str], source: &str) -> dx_testing::Run {
    let source = file(source);
    let source = source.to_string_lossy();
    let mut all = args.to_vec();
    all.push(source.as_ref());
    dx_testing::run(&file("DX_LDC"), &all, &[]).expect("ldc runs")
}

#[test]
fn compile_error_names_file_and_line() {
    let run = ldc(&["-c", "-o-"], "DX_COMPILE_ERROR");
    assert!(!run.status.success());
    assert!(
        run.combined()
            .contains("compile_error.d(3): Error: cannot implicitly convert"),
        "{}",
        run.combined()
    );
}

#[test]
fn missing_dependency_cannot_be_read() {
    let run = ldc(&["-c", "-o-"], "DX_MISSING_DEP");
    assert!(!run.status.success());
    assert!(
        run.combined()
            .contains("missing_dep.d(1): Error: unable to read module `module_name`"),
        "{}",
        run.combined()
    );
}

#[test]
fn failing_unittest_assert_reports_its_line() {
    let run = ldc(&["-unittest", "-c", "-o-"], "DX_FAILING_UNITTEST");
    assert!(!run.status.success());
    assert!(
        run.combined()
            .contains("failing_unittest.d(8): Error: static assert"),
        "{}",
        run.combined()
    );
}
