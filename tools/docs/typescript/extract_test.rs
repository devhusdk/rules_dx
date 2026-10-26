//! Checks the TypeScript fixture shard and the extractor's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::{decode_shard, validate_shard};

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn fixture() -> documentation_ir::proto::DocIr {
    let bytes = std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("read the fixture shard");
    decode_shard(&bytes).expect("the fixture shard decodes")
}

#[test]
fn the_fixture_shard_is_valid_and_prefixed() {
    let shard = fixture();
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "typescript");
    assert_eq!(shard.package, "geometry");
    assert!(!shard.symbols.is_empty());
    for symbol in &shard.symbols {
        assert!(
            symbol.id.starts_with("typescript:geometry:"),
            "{}",
            symbol.id
        );
    }
}

#[test]
fn overloads_are_numbered_and_reexports_link_back() {
    let shard = fixture();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert!(
        ids.contains(&"typescript:geometry:scale#1")
            && ids.contains(&"typescript:geometry:scale#2"),
        "the overloads of scale are numbered: {ids:?}"
    );
    let first = shard
        .symbols
        .iter()
        .find(|s| s.id == "typescript:geometry:scale#1")
        .expect("the first overload");
    let aliases = first.relations.as_ref().map(|r| r.reexported_as.clone());
    assert_eq!(
        aliases,
        Some(vec!["typescript:geometry:resize".to_owned()]),
        "the first overload lists the re-export alias"
    );
    assert!(!ids.contains(&"typescript:geometry:resize"));
}

#[test]
fn sources_are_repo_relative() {
    let shard = fixture();
    for symbol in &shard.symbols {
        let source = symbol.source.as_ref().expect("every symbol has a source");
        assert!(
            source
                .file
                .starts_with("tools/docs/typescript/fixture/src/"),
            "{}: {}",
            symbol.id,
            source.file
        );
        assert!(!source.generated);
        assert!(source.line > 0);
    }
}

fn run_extract(dir: &Path, typedoc: &Path) -> (i32, String) {
    let output = Command::new(runfile("DX_TYPESCRIPT_EXTRACT"))
        .args(["--typedoc"])
        .arg(typedoc)
        .args(["--package", "geometry", "--out", "shard.ir.pb"])
        .current_dir(dir)
        .output()
        .expect("run typescript_extract");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_binary_rejects_a_wrong_schema_version_and_writes_no_shard() {
    let dir = dx_testing::mkscratch("ts-extract").expect("scratch");
    let json = std::fs::read_to_string(runfile("DX_FIXTURE_TYPEDOC")).expect("typedoc json");
    assert!(json.contains("\"schemaVersion\": \"2.0\""));
    let old = dir.join("old.json");
    std::fs::write(
        &old,
        json.replace("\"schemaVersion\": \"2.0\"", "\"schemaVersion\": \"1.0\""),
    )
    .expect("write");
    let (code, stderr) = run_extract(&dir, &old);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("schemaVersion '1.0'"), "{stderr}");
    assert!(!dir.join("shard.ir.pb").exists());
    let (code, stderr) = run_extract(&dir, &runfile("DX_FIXTURE_TYPEDOC"));
    assert_eq!(code, 0, "{stderr}");
    assert!(dir.join("shard.ir.pb").exists());
}

#[test]
fn namespaces_merged_with_values_get_their_own_ids() {
    let shard = fixture();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    for id in [
        "typescript:geometry:Canvas",
        "typescript:geometry:Canvas#namespace",
        "typescript:geometry:Canvas#namespace.Options",
        "typescript:geometry:draw",
        "typescript:geometry:draw#namespace",
        "typescript:geometry:draw#namespace.lineWidth",
    ] {
        assert!(ids.contains(&id), "{id} is missing: {ids:?}");
    }
}
