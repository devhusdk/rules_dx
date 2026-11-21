use dx_testing::read_runfiles;

const PINS: &str = "cli/prebuilt/pins.bzl";

fn pins_version(pins: &str) -> String {
    pins.lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{PINS} has no DX_VERSION line"))
        .to_owned()
}

#[test]
fn the_prebuilt_pins_are_schema_1_and_empty_on_main() {
    // This is the pins check on main. fill_pins writes PINS only into a release's source
    // archive, and release.yml's pins job checks those archive pins.
    let pins = read_runfiles(PINS);
    assert!(pins.lines().any(|line| line == "SCHEMA = 1"), "{PINS} is not SCHEMA = 1");
    assert!(pins.lines().any(|line| line == "PINS = {}"), "{PINS} has pins on main");
}

#[test]
fn the_prebuilt_version_matches_the_module_and_the_cli() {
    let version = pins_version(&read_runfiles(PINS));
    let module = read_runfiles("MODULE.bazel");
    assert!(
        module.contains(&format!(
            "module(\n    name = \"rules_dx\",\n    version = \"{version}\","
        )),
        "{PINS} DX_VERSION {version} differs from MODULE.bazel"
    );
    let cli = read_runfiles("cli/adopt/src/version.rs");
    assert!(
        cli.contains(&format!("pub const DX_VERSION: &str = \"{version}\";")),
        "{PINS} DX_VERSION {version} differs from cli/adopt/src/version.rs"
    );
}
