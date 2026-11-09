//! Checks that the fixture shard carries the `doc` text of the fixture's .avsc and .avdl files.

use std::path::PathBuf;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "avro:shop:";

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
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn the_fixture_shard_documents_the_avsc_and_the_avdl() {
    let shard = shard();
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "avro");
    assert_eq!(shard.package, "shop");
    // Enum symbols have no doc attribute in Avro; every other item in the fixture has one.
    for symbol in &shard.symbols {
        if symbol.kind != SymbolKind::EnumVariant as i32 {
            assert!(!symbol.doc_markdown.is_empty(), "{} has no doc", symbol.id);
        }
    }
    for (qualified, kind, doc, file) in [
        (
            "shop.v1.Order",
            SymbolKind::Class,
            "A customer order.",
            "tools/docs/avro/fixture/shop.avsc",
        ),
        (
            "shop.v1.Order.id",
            SymbolKind::Field,
            "The order id.",
            "tools/docs/avro/fixture/shop.avsc",
        ),
        (
            "shop.v1.Status",
            SymbolKind::Enum,
            "Where an order is in its life.",
            "tools/docs/avro/fixture/shop.avsc",
        ),
        (
            "shop.v1.Digest",
            SymbolKind::TypeAlias,
            "A 128 bit content hash.",
            "tools/docs/avro/fixture/shop.avsc",
        ),
        (
            "shop.v1.Line.sku",
            SymbolKind::Field,
            "What was bought.",
            "tools/docs/avro/fixture/service.avdl",
        ),
        (
            "shop.v1.NoSuchOrder",
            SymbolKind::Class,
            "Raised when an order id is unknown.",
            "tools/docs/avro/fixture/service.avdl",
        ),
    ] {
        let found = symbol(&shard, qualified);
        assert_eq!(found.kind, kind as i32, "{qualified}");
        assert_eq!(found.doc_markdown, doc, "{qualified}");
        let source = found.source.as_ref().expect("source");
        assert_eq!(source.file, file, "{qualified}");
        assert!(!source.generated, "{qualified}");
    }
    let status = symbol(&shard, "shop.v1.Order.status");
    assert_eq!(status.signature_text, "shop.v1.Status status");
}
