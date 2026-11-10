//! Checks that the fixture shard carries the `doc` text of the fixture's .avsc and .avdl files.

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::validate_shard;
use shard_test_support::shard;

const PREFIX: &str = "avro:shop:";

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    shard_test_support::symbol(shard, &format!("{PREFIX}{name}"))
}

#[test]
fn the_fixture_shard_documents_the_avsc_and_the_avdl() {
    let shard = shard("DX_FIXTURE_SHARD");
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
