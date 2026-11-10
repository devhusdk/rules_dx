//! Checks that the fixture shard carries the symbol graph docs and the source lines of the fixture module.

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::validate_shard;
use shard_test_support::shard;

const PREFIX: &str = "swift:shapes:";

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    shard_test_support::symbol(shard, &format!("{PREFIX}{name}"))
}

#[test]
fn the_fixture_shard_documents_the_module() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "swift");
    assert_eq!(shard.package, "shapes");
    for (qualified, kind, doc, file, line) in [
        (
            "Shapes.Circle",
            SymbolKind::Class,
            "A circle.",
            "Shape.swift",
            2,
        ),
        (
            "Shapes.Circle.radius",
            SymbolKind::Property,
            "The radius.",
            "Shape.swift",
            4,
        ),
        (
            "Shapes.Measured",
            SymbolKind::Interface,
            "Things that have an area.",
            "Shape.swift",
            20,
        ),
        (
            "Shapes.Unit",
            SymbolKind::Enum,
            "A length unit.",
            "Unit.swift",
            2,
        ),
        (
            "Shapes.Unit.centimetre",
            SymbolKind::EnumVariant,
            "Centimetres.",
            "Unit.swift",
            6,
        ),
        (
            "Shapes.Radius",
            SymbolKind::TypeAlias,
            "A radius in metres.",
            "Unit.swift",
            10,
        ),
        (
            "Shapes.metres~28_~3Ain~3A~29",
            SymbolKind::Function,
            "Converts a length to metres.",
            "Unit.swift",
            13,
        ),
    ] {
        let found = symbol(&shard, qualified);
        assert_eq!(found.kind, kind as i32, "{qualified}");
        assert_eq!(found.doc_markdown, doc, "{qualified}");
        let source = found.source.as_ref().expect("source");
        assert_eq!(
            source.file,
            format!("tools/docs/swift/fixture/src/{file}"),
            "{qualified}"
        );
        assert_eq!(source.line, line, "{qualified}");
        assert!(!source.generated, "{qualified}");
    }
    let scale = symbol(&shard, "Shapes.Circle.scale~28by~3A~29");
    assert_eq!(scale.params[0].name, "factor");
    assert_eq!(scale.params[0].doc, "How much to grow.");
    assert_eq!(
        scale.returns.as_ref().expect("returns").doc,
        "The grown circle."
    );
}

#[test]
fn the_fixture_shard_documents_members_added_to_other_modules_types() {
    let shard = shard("DX_FIXTURE_SHARD");
    let shout = symbol(&shard, "Swift.String.shout~28~29");
    assert_eq!(shout.kind, SymbolKind::Method as i32);
    assert_eq!(shout.doc_markdown, "Upper-cases the string.");
    let source = shout.source.as_ref().expect("source");
    assert_eq!(source.file, "tools/docs/swift/fixture/src/Text.swift");
    assert_eq!(source.line, 3);
    let extends = shout
        .extensions
        .iter()
        .find(|extension| extension.key == "swift.extends")
        .expect("extends");
    assert_eq!(extends.value, b"s:SS");
}

#[test]
fn docc_symbol_links_become_dx_links() {
    let shard = shard("DX_FIXTURE_SHARD");
    let area = symbol(&shard, "Shapes.Measured.area");
    assert_eq!(
        area.doc_markdown,
        "The area of a [Circle](dx:swift:shapes:Shapes.Circle) or a ``Missing``."
    );
}
