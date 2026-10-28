//! Checks the Python fixture shard produced by `python_doc_shard`.

use std::path::PathBuf;

use documentation_ir::proto::DocIr;
use documentation_ir::{decode_shard, validate_shard};

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard() -> DocIr {
    let bytes = std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

#[test]
fn the_fixture_shard_lists_the_public_api() {
    let shard = shard();
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "python");
    assert_eq!(shard.package, "mylib");
    let mut ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    assert!(ids.contains(&"python:mylib:shapes.area"), "{ids:?}");
    assert!(ids.contains(&"python:mylib:gen.generated_name"), "{ids:?}");
    assert!(!ids.iter().any(|id| id.contains("_private")), "{ids:?}");
}

#[test]
fn signatures_keep_the_bare_star() {
    let shard = shard();
    let area = shard
        .symbols
        .iter()
        .find(|s| s.id == "python:mylib:shapes.area")
        .expect("area");
    assert!(
        area.signature_text.contains("width, *, height"),
        "{}",
        area.signature_text
    );
    assert!(area.doc_markdown.contains("Computes an area."));
}

#[test]
fn sources_are_repo_relative_and_generated_modules_are_flagged() {
    let shard = shard();
    let text = std::fs::read_to_string(runfile("DX_FIXTURE_SHAPES_PY")).expect("read source");
    let line = text
        .lines()
        .position(|line| line.starts_with("def area"))
        .expect("def area") as u64
        + 1;
    for symbol in &shard.symbols {
        let source = symbol.source.as_ref().expect("source");
        assert!(
            source
                .file
                .starts_with("tools/docs/python/fixture/src/mylib/"),
            "{}",
            source.file
        );
        assert_eq!(source.generated, source.file.ends_with("/gen.py"));
        if symbol.id == "python:mylib:shapes.area" {
            assert_eq!(source.line, line);
        }
    }
}

#[test]
fn the_numpy_shard_supplies_param_types_and_docs() {
    let bytes = std::fs::read(runfile("DX_FIXTURE_NUMPY_SHARD")).expect("read the shard");
    let shard = decode_shard(&bytes).expect("the shard decodes");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.package, "npylib");
    let scale = shard
        .symbols
        .iter()
        .find(|s| s.id == "python:npylib:scale")
        .expect("scale");
    assert_eq!(scale.params[0].r#type, "list of int");
    assert_eq!(scale.params[1].doc, "The factor.");
}
