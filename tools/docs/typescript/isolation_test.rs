//! Checks that consumer TypeDoc configuration cannot inject plugins into the docs rule.

use std::path::PathBuf;

use documentation_ir::{decode_shard, validate_shard};

const MARKERS: [&str; 3] = [
    "DX_SENTINEL_TSCONFIG",
    "DX_SENTINEL_BASE",
    "DX_SENTINEL_PACKAGE",
];

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn read(var: &str) -> String {
    std::fs::read_to_string(runfile(var)).expect("read the TypeDoc JSON")
}

#[test]
fn the_stock_cli_loads_every_fixture_plugin() {
    for (var, marker) in [
        ("DX_HOSTILE_CONTROL_TSCONFIG", MARKERS[0]),
        ("DX_HOSTILE_CONTROL_BASE", MARKERS[1]),
        ("DX_HOSTILE_CONTROL_PACKAGE", MARKERS[2]),
    ] {
        assert!(read(var).contains(marker), "control {var} lacks {marker}");
    }
}

#[test]
fn the_rule_loads_no_plugin() {
    let docs = read("DX_HOSTILE_TYPEDOC");
    for marker in MARKERS {
        assert!(!docs.contains(marker), "plugin ran: {marker}");
    }
    assert!(docs.contains("\"hostile\""));
}

#[test]
fn the_rule_ignores_consumer_output_options() {
    let json = runfile("DX_HOSTILE_TYPEDOC");
    let dir = json.parent().expect("output directory");
    assert!(!dir.join("html").exists(), "typedocOptions.out was honored");
    assert!(!dir.join("index.html").exists());
}

#[test]
fn the_hostile_shard_is_valid() {
    let bytes = std::fs::read(runfile("DX_HOSTILE_SHARD")).expect("read the shard");
    let shard = decode_shard(&bytes).expect("the shard decodes");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.package, "hostile");
    assert!(!shard.symbols.is_empty());
}
