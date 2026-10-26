fn check_one_env(
    bin_rel: &str,
    expected: &str,
    args: &str,
    envs: &[(&str, &str)],
) -> Result<(), String> {
    let binary = dx_testing::resolve_runfiles(bin_rel);
    let argv: Vec<&str> = args.split_whitespace().collect();
    let output = std::process::Command::new(&binary)
        .args(&argv)
        .envs(envs.iter().copied())
        .output()
        .map_err(|e| format!("ide binary {} must execute: {e}", binary.display()))?;
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        return Err(format!(
            "ide binary {} exited with {}\n{combined}",
            binary.display(),
            output
                .status
                .code()
                .map_or("a signal".to_string(), |c| format!("code {c}")),
        ));
    }
    if !combined.contains(expected) {
        return Err(format!(
            "ide acquisition missing {expected} from {}\n{combined}",
            binary.display(),
        ));
    }
    Ok(())
}

fn check_one(bin_rel: &str, expected: &str, args: &str) -> Result<(), String> {
    check_one_env(bin_rel, expected, args, &[])
}

#[test]
fn ide_acquisition_answers_version() {
    let bin = std::env::var("DX_IDE_BIN").expect("DX_IDE_BIN must name the ide binary");
    let expected =
        std::env::var("DX_IDE_EXPECTED").expect("DX_IDE_EXPECTED must hold the version prefix");
    let args = std::env::var("DX_IDE_ARGS").unwrap_or_default();
    check_one(&bin, &expected, &args).unwrap();
    if let Ok(bin_two) = std::env::var("DX_IDE_BIN_TWO") {
        let expected_two = std::env::var("DX_IDE_EXPECTED_TWO")
            .expect("DX_IDE_EXPECTED_TWO must be set with BIN_TWO");
        let args_two = std::env::var("DX_IDE_ARGS_TWO").unwrap_or_default();
        check_one(&bin_two, &expected_two, &args_two).unwrap();
    }
}

/// Child of `nonzero_exit_is_rejected`: prints a version then fails.
#[test]
fn fake_tool() {
    if std::env::var_os("DX_IDE_FAKE_TOOL").is_some() {
        println!("v9");
        std::process::exit(1);
    }
}

#[test]
fn nonzero_exit_is_rejected() {
    let exe = std::env::current_exe().unwrap();
    let err = check_one_env(
        exe.to_str().unwrap(),
        "v9",
        "--exact fake_tool --nocapture",
        &[("DX_IDE_FAKE_TOOL", "1")],
    )
    .unwrap_err();
    assert!(err.contains("code 1"), "unexpected error: {err}");
}
