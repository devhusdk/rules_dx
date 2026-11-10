//! Checks that the fixture shard carries the symbol graph docs and the source lines of the fixture module.

use std::path::PathBuf;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "swift:shapes:";

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard() -> DocIr {
    let bytes = std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let id = format!("{PREFIX}{qualified}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| {
            panic!(
                "{id} is missing from {:?}",
                shard.symbols.iter().map(|s| &s.id).collect::<Vec<_>>()
            )
        })
}

#[test]
fn the_fixture_shard_documents_the_module() {
    let shard = shard();
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
