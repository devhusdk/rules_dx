//! Checks that the fixture shard carries the comments of the fixture's .proto files.

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::validate_shard;
use shard_test_support::shard;

const PREFIX: &str = "protobuf:shop:";

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    shard_test_support::symbol(shard, &format!("{PREFIX}{name}"))
}

#[test]
fn the_fixture_shard_documents_every_item_with_its_comment() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "protobuf");
    assert_eq!(shard.package, "shop");
    // Comments are lost when protoc reads the library's own descriptor set, so an empty doc
    // text anywhere means the build fed it back in.
    for symbol in &shard.symbols {
        assert!(!symbol.doc_markdown.is_empty(), "{} has no doc", symbol.id);
    }
    for (qualified, kind, doc) in [
        ("shop.v1.Order", SymbolKind::Class, "A customer order."),
        (
            "shop.v1.Order.Line",
            SymbolKind::Class,
            "One line of an order.",
        ),
        ("shop.v1.Order.id", SymbolKind::Field, "The order id."),
        ("shop.v1.Shop", SymbolKind::Interface, "The order API."),
        (
            "shop.v1.Shop.GetOrder",
            SymbolKind::Method,
            "Reads one order.",
        ),
    ] {
        let found = symbol(&shard, qualified);
        assert_eq!(found.kind, kind as i32, "{qualified}");
        assert_eq!(found.doc_markdown, doc, "{qualified}");
    }
    let map = symbol(&shard, "shop.v1.Order.labels");
    assert_eq!(map.signature_text, "map<string, string> labels = 4;");
    assert!(!shard.symbols.iter().any(|s| s.id.contains("LabelsEntry")));
    let watch = symbol(&shard, "shop.v1.Shop.Watch");
    assert_eq!(
        watch.signature_text,
        "rpc Watch(stream shop.v1.GetOrderRequest) returns (stream shop.v1.Order);"
    );
    let status = symbol(&shard, "shop.v1.Order.status");
    assert_eq!(status.signature_text, "shop.v1.Status status = 2;");
}

#[test]
fn the_imported_proto_is_not_documented_by_the_importing_shard() {
    let shard = shard("DX_FIXTURE_SHARD");
    assert!(!shard.symbols.iter().any(|s| s.id.contains("Stamp")));
    let source = symbol(&shard, "shop.v1.Order")
        .source
        .clone()
        .expect("source");
    assert_eq!(source.file, "tools/docs/protobuf/fixture/shop.proto");
    assert_eq!(source.line, 8);
    assert!(!source.generated);
}
