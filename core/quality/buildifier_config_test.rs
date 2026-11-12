//! Buildifier accepts every quality config without an unexpected warning about its own settings.

use dx_testing::{env_file, env_files, path_text, run, scratch_dir, write};

#[test]
fn configs_have_no_unexpected_warning() {
    let buildifier = env_file("BUILDIFIER");
    let build = scratch_dir("buildifier_config").join("BUILD.bazel");
    write(&build, "load(\"//a:b.bzl\", \"c\")\n");
    let build = path_text(&build);
    let configs = env_files("CONFIGS");
    assert_eq!(configs.len(), 2, "two configs");
    for config in configs {
        let flag = format!("-config={}", path_text(&config));
        let out = run(&buildifier, &[&flag, "-mode=check", "-lint=warn", &build], &[]).expect("run buildifier");
        let text = out.combined();
        assert!(
            out.status.success() || !text.contains("unexpected warning"),
            "{}: {}",
            config.display(),
            text.chars().take(400).collect::<String>()
        );
    }
}
