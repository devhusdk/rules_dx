#[test]
fn sources_match_the_pinned_biome_and_the_app_config() {
    let tool =
        dx_testing::resolve_runfiles(&std::env::var("DX_BIOME").expect("DX_BIOME names the tool"));
    let config = dx_testing::resolve_runfiles(
        &std::env::var("DX_CONFIG").expect("DX_CONFIG names the config"),
    );
    let config_arg = format!("--config-path={}", config.to_string_lossy());
    let sources = std::env::var("DX_FORMAT_SRCS").expect("DX_FORMAT_SRCS lists the sources");
    let mut failures = Vec::new();
    let mut checked = 0;
    for rel in sources.split_whitespace() {
        let file = dx_testing::resolve_runfiles(rel);
        let file_arg = file.to_string_lossy().into_owned();
        let run = dx_testing::run(
            &tool,
            &["check", config_arg.as_str(), file_arg.as_str()],
            &[],
        )
        .expect("biome runs");
        checked += 1;
        if !run.status.success() {
            failures.push(format!("{rel}\n{}", run.combined()));
        }
    }
    assert!(checked > 0, "no sources were listed");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
