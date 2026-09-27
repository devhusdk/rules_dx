fn check_one(bin_rel: &str, expected: &str, args: &str) {
    let binary = dx_testing::resolve_runfiles(bin_rel);
    let argv: Vec<&str> = args.split_whitespace().collect();
    let output = std::process::Command::new(&binary)
        .args(&argv)
        .output()
        .expect("ide binary must execute");
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert!(
        combined.contains(expected),
        "ide acquisition missing {expected} from {}\n{combined}",
        binary.display(),
    );
}

#[test]
fn ide_acquisition_answers_version() {
    let bin = std::env::var("DX_IDE_BIN").expect("DX_IDE_BIN must name the ide binary");
    let expected =
        std::env::var("DX_IDE_EXPECTED").expect("DX_IDE_EXPECTED must hold the version prefix");
    let args = std::env::var("DX_IDE_ARGS").unwrap_or_default();
    check_one(&bin, &expected, &args);
    if let Ok(bin_two) = std::env::var("DX_IDE_BIN_TWO") {
        let expected_two = std::env::var("DX_IDE_EXPECTED_TWO")
            .expect("DX_IDE_EXPECTED_TWO must be set with BIN_TWO");
        let args_two = std::env::var("DX_IDE_ARGS_TWO").unwrap_or_default();
        check_one(&bin_two, &expected_two, &args_two);
    }
}
