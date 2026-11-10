use dx_preset::{DEFAULT_REPO, Layer, render_layer};

#[test]
fn dx_settings_are_in_the_correctness_layer() {
    let layer = render_layer(Layer::Correctness, DEFAULT_REPO);
    let lines: Vec<&str> = layer.lines().collect();
    for verb in ["build", "test", "coverage", "run"] {
        for setting in dx_plan::plan::settings_for(DEFAULT_REPO, verb) {
            let line = format!("build {setting}");
            assert!(
                lines.contains(&line.as_str()),
                "dx adds `{setting}` to `{verb}`, but the correctness layer has no `{line}` line; \
                 add it to a correctness group in tools/bazelrc/src/lib.rs"
            );
        }
    }
}
