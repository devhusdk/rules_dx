use dx_testing::{env_file, env_text, path_text};

#[test]
fn sources_match_the_pinned_formatter() {
    let tool = env_file("DX_FORMAT_TOOL");
    let config = std::env::var("DX_FORMAT_CONFIG")
        .map(|rel| path_text(&dx_testing::resolve_runfiles(&rel)))
        .unwrap_or_default();
    let template = std::env::var("DX_FORMAT_ARGS").unwrap_or_default();
    let sources = env_text("DX_FORMAT_SRCS");
    let mut checked = 0;
    let mut failures = Vec::new();
    for rel in sources.split_whitespace() {
        let file = path_text(&dx_testing::resolve_runfiles(rel));
        let args: Vec<String> = template
            .split_whitespace()
            .map(|word| word.replace("{config}", &config).replace("{src}", &file))
            .collect();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = dx_testing::run(&tool, &args, &[]).expect("formatter runs");
        checked += 1;
        if !run.status.success() {
            failures.push(format!("{rel}\n{}", run.combined()));
        }
    }
    assert!(checked > 0, "no sources were listed");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
