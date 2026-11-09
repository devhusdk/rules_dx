use dx_testing::read_runfiles;

const ASSETS: [&str; 5] = [
    "dx-linux-x86_64",
    "dx-linux-arm64",
    "dx-macos-arm64",
    "dx-windows-x86_64",
    "dx-windows-arm64",
];

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn expected() -> Vec<String> {
    sorted(ASSETS.iter().map(|asset| (*asset).to_owned()).collect())
}

const PINS: &str = "cli/prebuilt/pins.bzl";

fn pins_version(pins: &str) -> String {
    pins.lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{PINS} has no DX_VERSION line"))
        .to_owned()
}

fn pinned_assets(pins: &str) -> Vec<String> {
    let start = pins.find("SHA256 = {\n").expect("SHA256 map") + "SHA256 = {\n".len();
    let end = pins[start..].find("\n}").expect("SHA256 map end") + start;
    pins[start..end]
        .lines()
        .map(|line| {
            let (name, hash) = line
                .trim()
                .split_once(": ")
                .unwrap_or_else(|| panic!("{PINS} entry {line:?}"));
            let hash = hash.trim_end_matches(',').trim_matches('"');
            assert!(
                hash.is_empty()
                    || (hash.len() == 64
                        && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))),
                "{PINS} pin for {name} is not a sha256"
            );
            name.trim_matches('"').to_owned()
        })
        .collect()
}

#[test]
fn the_prebuilt_pins_name_every_asset_once_in_order() {
    let pins = read_runfiles(PINS);
    let names = pinned_assets(&pins);
    assert_eq!(
        names,
        sorted(names.clone()),
        "{PINS} entries are not sorted"
    );
    assert_eq!(names, expected(), "{PINS} keys differ from ASSETS");
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
