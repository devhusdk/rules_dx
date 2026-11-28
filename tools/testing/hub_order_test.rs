fn toolchain_values(build: &str, attr: &str) -> Vec<String> {
    let prefix = format!("{attr} = \"");
    let mut values = Vec::new();
    let mut in_toolchain = false;
    for line in build.lines() {
        if line.starts_with("toolchain(") {
            in_toolchain = true;
        } else if line.starts_with(')') {
            in_toolchain = false;
        } else if in_toolchain && let Some(rest) = line.trim().strip_prefix(prefix.as_str()) {
            values.extend(rest.split('"').next().map(str::to_owned));
        }
    }
    values
}

fn hub_build() -> String {
    let key = dx_testing::env_text("DX_HUB_BUILD");
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{key}: {error}"));
    dx_testing::read_text(&path)
}

fn env_words(name: &str) -> Vec<String> {
    dx_testing::env_text(name)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_hub_registers_the_named_toolchains_in_order() {
    let mut found = toolchain_values(&hub_build(), "name");
    found.sort();
    assert_eq!(
        found,
        env_words("DX_HUB_NAMES"),
        "the toolchains of //:all in registration order"
    );
}

#[test]
fn the_hub_toolchains_have_the_named_toolchain_types() {
    let mut found = toolchain_values(&hub_build(), "toolchain_type");
    found.sort();
    found.dedup();
    assert_eq!(
        found,
        env_words("DX_HUB_TOOLCHAIN_TYPES"),
        "the toolchain types of the hub's toolchains"
    );
}

#[test]
fn toolchain_values_take_each_toolchain_block() {
    let build = "load(\"@x//:defs.bzl\", \"x\")\n\ntoolchain(\n    name = \"b_toolchain\",\n    toolchain = \"@b//:t\",\n    toolchain_type = \"//b:type\",\n)\n\nx(\n    name = \"other\",\n    toolchain_type = \"//x:type\",\n)\n\ntoolchain(\n    name = \"a_toolchain\",\n)\n";
    assert_eq!(
        toolchain_values(build, "name"),
        vec!["b_toolchain", "a_toolchain"]
    );
    assert_eq!(toolchain_values(build, "toolchain_type"), vec!["//b:type"]);
}
