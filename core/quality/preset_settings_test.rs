use dx_preset::{DEFAULT_REPO, Layer, render_layer};

#[test]
fn dx_settings_are_the_rust_setup_flags() {
    let rust: Vec<String> = dx_families::setup_flags(["rust"])
        .into_iter()
        .flat_map(|(_, flags)| flags)
        .collect();
    let layer = render_layer(Layer::Correctness, DEFAULT_REPO);
    for verb in ["build", "test", "coverage", "run"] {
        for setting in dx_plan::plan::settings_for(DEFAULT_REPO, verb) {
            let line = format!("build {setting}");
            assert!(
                rust.contains(&line),
                "dx adds `{setting}` to `{verb}`, but rust/lang.json has no `{line}` setup flag"
            );
            assert!(
                !layer.contains(&setting),
                "`{setting}` needs @rules_rust; keep it out of the shared correctness layer"
            );
        }
    }
}
