fn toolchain_names(build: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_toolchain = false;
    for line in build.lines() {
        if line.starts_with("toolchain(") {
            in_toolchain = true;
        } else if in_toolchain && let Some(rest) = line.trim().strip_prefix("name = \"") {
            names.extend(rest.split('"').next().map(str::to_owned));
            in_toolchain = false;
        }
    }
    names
}

#[test]
fn the_hub_registers_the_named_toolchains_in_order() {
    let key = dx_testing::env_text("DX_HUB_BUILD");
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{key}: {error}"));
    let build = dx_testing::read_text(&path);
    let mut found = toolchain_names(&build);
    found.sort();
    let want: Vec<String> = dx_testing::env_text("DX_HUB_NAMES")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        found, want,
        "the toolchains of //:all in registration order"
    );
}

#[test]
fn toolchain_names_take_each_toolchain_block() {
    let build = "load(\"@x//:defs.bzl\", \"x\")\n\ntoolchain(\n    name = \"b_toolchain\",\n    toolchain = \"@b//:t\",\n)\n\nx(\n    name = \"other\",\n)\n\ntoolchain(\n    name = \"a_toolchain\",\n)\n";
    assert_eq!(toolchain_names(build), vec!["b_toolchain", "a_toolchain"]);
}
