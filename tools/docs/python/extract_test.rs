//! Checks the Python fixture shard produced by `python_doc_shard`.

use documentation_ir::validate_shard;
use shard_test_support::{line_of, shard, symbol};

#[test]
fn the_fixture_shard_lists_the_public_api() {
    let shard = shard("DX_FIXTURE_SHARD");
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
    let shard = shard("DX_FIXTURE_SHARD");
    let area = symbol(&shard, "python:mylib:shapes.area");
    assert!(
        area.signature_text.contains("width, *, height"),
        "{}",
        area.signature_text
    );
    assert!(area.doc_markdown.contains("Computes an area."));
}

#[test]
fn sources_are_repo_relative_and_generated_modules_are_flagged() {
    let shard = shard("DX_FIXTURE_SHARD");
    let line = line_of("DX_FIXTURE_SHAPES_PY", "def area");
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
    let shard = shard("DX_FIXTURE_NUMPY_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.package, "npylib");
    let scale = symbol(&shard, "python:npylib:scale");
    assert_eq!(scale.params[0].r#type, "list of int");
    assert_eq!(scale.params[1].doc, "The factor.");
}
