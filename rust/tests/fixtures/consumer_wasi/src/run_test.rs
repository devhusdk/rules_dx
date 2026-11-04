use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Output;

fn path(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
    dx_testing::resolve_runfiles(&rel)
}

fn run(args: &[&str], env: &[(&str, &str)], preopens: &[(&str, &str)]) -> Output {
    assert_eq!(std::env::var("DX_WASI_HOSTILE").as_deref(), Ok("1"));
    let input = std::env::var("DX_WASI_INPUT").expect("DX_WASI_INPUT must be set");
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    let preopens: Vec<(String, PathBuf)> = preopens
        .iter()
        .map(|(guest, _)| (guest.to_string(), PathBuf::from(&input)))
        .collect();
    wasi_run::run(
        &path("DX_WASI_RUNTIME"),
        &path("DX_WASI_MODULE"),
        &args,
        &env,
        &preopens,
    )
}

fn expect(output: &Output, code: i32, lines: [&str; 6]) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(code), "stderr: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut want = lines.join("\n");
    want.push_str(
        "\nmodule=wasm\nbuild_target=wasm32-wasip1\nbuild_script=native\nproc_macro=native",
    );
    assert_eq!(
        stdout.trim_end_matches(['\r', '\n']),
        want,
        "stderr: {stderr}"
    );
}

const DEFAULT: [&str; 6] = [
    "argv=[]",
    "declared=<unset>",
    "wrapped=<unset>",
    "hostile=hidden",
    "data=no-access",
    "probe=no-access",
];

#[test]
fn default_run() {
    expect(&run(&[], &[], &[]), 0, DEFAULT);
}

#[test]
fn declared_capabilities() {
    let output = run(
        &["one", "two"],
        &[
            ("DX_WASI_DECLARED", "declared-value"),
            ("DX_WASI_PROBE", "/data/input.txt"),
        ],
        &[("/data", "")],
    );
    expect(
        &output,
        0,
        [
            r#"argv=["one", "two"]"#,
            "declared=declared-value",
            "wrapped=<unset>",
            "hostile=hidden",
            "data=staged input",
            "probe=staged input",
        ],
    );
}

#[test]
fn shared_preopen() {
    let output = run(
        &["one", "two"],
        &[
            ("DX_WASI_DECLARED", "declared-value"),
            ("DX_WASI_PROBE", "/b/input.txt"),
        ],
        &[("/a", ""), ("/b", "")],
    );
    expect(
        &output,
        0,
        [
            r#"argv=["one", "two"]"#,
            "declared=declared-value",
            "wrapped=<unset>",
            "hostile=hidden",
            "data=no-access",
            "probe=staged input",
        ],
    );
}

#[test]
fn dollar_literals() {
    let output = run(
        &["--price", "$5", "$(BINDIR)"],
        &[
            ("DX_WASI_DECLARED", "$HOME"),
            ("DX_WASI_PROBE", "/d$ta/input.txt"),
        ],
        &[("/d$ta", "")],
    );
    expect(
        &output,
        0,
        [
            r#"argv=["--price", "$5", "$(BINDIR)"]"#,
            "declared=$HOME",
            "wrapped=<unset>",
            "hostile=hidden",
            "data=no-access",
            "probe=staged input",
        ],
    );
}

#[test]
fn custom_exit() {
    expect(&run(&[], &[("DX_WASI_EXIT", "3")], &[]), 3, DEFAULT);
}
