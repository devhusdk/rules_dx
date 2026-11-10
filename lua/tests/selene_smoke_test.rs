//! The source-built selene parses Lua 5.2 to 5.4 syntax and still reports lints with the staged
//! default config.

use dx_testing::{env_file, path_text, run};

fn selene(source: &str) -> dx_testing::Run {
    let config = path_text(&env_file("SELENE_CONFIG"));
    let source = path_text(&env_file(source));
    run(
        &env_file("SELENE"),
        &[
            "--config",
            &config,
            "--display-style",
            "json2",
            "--color",
            "Never",
            "--no-exclude",
            "--num-threads",
            "1",
            &source,
        ],
        &[],
    )
    .expect("selene runs")
}

#[test]
fn version_is_pinned() {
    let out = run(&env_file("SELENE"), &["--version"], &[]).expect("selene runs");
    assert!(out.status.success(), "{}", out.combined());
    assert_eq!(out.stdout.trim(), "selene 0.32.0");
}

#[test]
fn lua54_source_is_clean() {
    let out = selene("LUA54_VALID");
    assert!(out.status.success(), "lua54 fixture rejected:\n{}", out.combined());
    assert!(
        out.stdout
            .contains(r#""type":"Summary","errors":0,"warnings":0,"parse_errors":0"#),
        "lua54 fixture not clean:\n{}",
        out.combined()
    );
}

#[test]
fn undefined_variable_is_reported() {
    let out = selene("UNDEFINED_VARIABLE");
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains(r#""code":"undefined_variable""#),
        "undefined_variable not reported:\n{}",
        out.combined()
    );
}
