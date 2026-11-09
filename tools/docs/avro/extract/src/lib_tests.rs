use super::*;

const P: &str = "avro:shop:";

fn options() -> Options {
    Options {
        package: "shop".to_owned(),
        out: "out.ir.pb".to_owned(),
        ..Options::default()
    }
}

fn shard_of(schemas: &[&str]) -> DocIr {
    let inputs: Vec<(String, String, String)> = schemas
        .iter()
        .map(|text| ((*text).to_owned(), "shop.avsc".to_owned(), "shop.avsc".to_owned()))
        .collect();
    convert(&inputs, &options()).expect("converts")
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let id = format!("{P}{qualified}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

const ORDER: &str = r#"{
  "type": "record",
  "name": "Order",
  "namespace": "shop.v1",
  "doc": "A customer order.",
  "fields": [
    {"name": "id", "type": "string", "doc": "The order id."},
    {"name": "tags", "type": {"type": "array", "items": "string"}},
    {"name": "note", "type": ["null", "string"], "default": null, "doc": "Free text."},
    {"name": "status", "type": {
      "type": "enum", "name": "Status", "doc": "Order state.", "symbols": ["OPEN", "SHIPPED"]
    }},
    {"name": "digest", "type": {"type": "fixed", "name": "Digest", "size": 16, "doc": "A hash."}},
    {"name": "attrs", "type": {"type": "map", "values": "long"}}
  ]
}"#;

#[test]
fn every_kind_maps_to_its_symbol_kind_with_its_doc() {
    let shard = shard_of(&[ORDER]);
    for (qualified, kind, doc) in [
        ("shop.v1.Order", SymbolKind::Class, "A customer order."),
        ("shop.v1.Order.id", SymbolKind::Field, "The order id."),
        ("shop.v1.Status", SymbolKind::Enum, "Order state."),
        ("shop.v1.Digest", SymbolKind::TypeAlias, "A hash."),
    ] {
        let found = symbol(&shard, qualified);
        assert_eq!(found.kind, kind as i32, "{qualified}");
        assert_eq!(found.doc_markdown, doc, "{qualified}");
    }
    let id = symbol(&shard, "shop.v1.Order.id");
    assert_eq!(
        id.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec![format!("{P}shop.v1.Order")])
    );
    let variant = symbol(&shard, "shop.v1.Status.OPEN");
    assert_eq!(variant.kind, SymbolKind::EnumVariant as i32);
    assert_eq!(
        variant.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec![format!("{P}shop.v1.Status")])
    );
}

#[test]
fn signatures_show_types_defaults_and_sizes() {
    let shard = shard_of(&[ORDER]);
    assert_eq!(symbol(&shard, "shop.v1.Order").signature_text, "record Order");
    assert_eq!(
        symbol(&shard, "shop.v1.Order.tags").signature_text,
        "array<string> tags"
    );
    assert_eq!(
        symbol(&shard, "shop.v1.Order.note").signature_text,
        "null | string note = null"
    );
    assert_eq!(
        symbol(&shard, "shop.v1.Order.attrs").signature_text,
        "map<string, long> attrs"
    );
    assert_eq!(
        symbol(&shard, "shop.v1.Order.status").signature_text,
        "shop.v1.Status status"
    );
    assert_eq!(
        symbol(&shard, "shop.v1.Digest").signature_text,
        "fixed Digest(16)"
    );
}

#[test]
fn a_missing_doc_gives_an_empty_doc_marked_incomplete() {
    let shard = shard_of(&[ORDER]);
    let tags = symbol(&shard, "shop.v1.Order.tags");
    assert_eq!(tags.doc_markdown, "");
    assert!(tags.extensions.iter().any(|e| e.key == KEY_INCOMPLETE));
    let documented = symbol(&shard, "shop.v1.Order.id");
    assert!(documented.extensions.is_empty());
}

#[test]
fn a_protocol_message_is_a_method_of_its_protocol() {
    let protocol = r#"{
      "protocol": "Shop", "namespace": "shop.v1", "doc": "The order API.",
      "types": [{"type": "error", "name": "NoSuchOrder", "doc": "Raised for an unknown id.",
                 "fields": [{"name": "id", "type": "string"}]}],
      "messages": {
        "getOrder": {"doc": "Reads one order.",
          "request": [{"name": "id", "type": "string", "doc": "The id."}],
          "response": "string", "errors": ["NoSuchOrder"]}
      }
    }"#;
    let shard = shard_of(&[protocol]);
    let shop = symbol(&shard, "shop.v1.Shop");
    assert_eq!(shop.kind, SymbolKind::Interface as i32);
    assert_eq!(shop.doc_markdown, "The order API.");
    let method = symbol(&shard, "shop.v1.Shop.getOrder");
    assert_eq!(method.kind, SymbolKind::Method as i32);
    assert_eq!(method.doc_markdown, "Reads one order.");
    assert_eq!(
        method.signature_text,
        "getOrder(string id) returns string throws NoSuchOrder"
    );
    assert_eq!(method.params.len(), 1);
    assert_eq!(symbol(&shard, "shop.v1.NoSuchOrder").kind, SymbolKind::Class as i32);
}

#[test]
fn namespaces_come_from_the_name_the_field_or_the_enclosing_type() {
    let schema = r#"{"type": "record", "name": "a.b.Outer", "doc": "d", "fields": [
      {"name": "inner", "type": {"type": "record", "name": "Inner", "doc": "i", "fields": []}},
      {"name": "other", "type": {"type": "fixed", "name": "Id", "namespace": "z", "size": 4, "doc": "f"}}
    ]}"#;
    let shard = shard_of(&[schema]);
    symbol(&shard, "a.b.Outer");
    symbol(&shard, "a.b.Inner");
    symbol(&shard, "z.Id");
}

#[test]
fn a_type_without_a_namespace_has_no_prefix() {
    let schema = r#"{"type": "enum", "name": "Color", "doc": "c", "symbols": ["RED"]}"#;
    let shard = shard_of(&[schema]);
    symbol(&shard, "Color");
    symbol(&shard, "Color.RED");
}

#[test]
fn the_documented_copy_of_a_repeated_type_wins() {
    let bare = r#"{"type": "enum", "name": "Color", "symbols": ["RED"]}"#;
    let documented = r#"{"type": "enum", "name": "Color", "doc": "Paint.", "symbols": ["RED"]}"#;
    for order in [[bare, documented], [documented, bare]] {
        let shard = shard_of(&order);
        assert_eq!(symbol(&shard, "Color").doc_markdown, "Paint.");
    }
}

#[test]
fn lines_point_at_the_name_in_the_source() {
    let shard = shard_of(&[ORDER]);
    assert_eq!(symbol(&shard, "shop.v1.Order").source.as_ref().map(|s| s.line), Some(3));
    assert_eq!(symbol(&shard, "shop.v1.Order.id").source.as_ref().map(|s| s.line), Some(7));
}

#[test]
fn bad_input_is_rejected_with_a_reason() {
    let inputs = vec![("{".to_owned(), "x.avsc".to_owned(), "x.avsc".to_owned())];
    let error = convert(&inputs, &options()).expect_err("malformed JSON");
    assert!(error.contains("not valid JSON"), "{error}");
    let inputs = vec![("\"string\"".to_owned(), "x.avsc".to_owned(), "x.avsc".to_owned())];
    let error = convert(&inputs, &options()).expect_err("no named type");
    assert!(error.contains("no named Avro type"), "{error}");
    let mut bad = options();
    bad.package = "a b".to_owned();
    let error = convert(&[], &bad).expect_err("package");
    assert!(error.contains("invalid package"), "{error}");
}

#[test]
fn parse_reads_flags_and_rejects_bad_ones() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let parsed = parse(&args(&[
        "--package", "shop", "--out", "o", "--schema", "a.avsc=src/a.avsc", "--schema-dir",
        "tree=src/b.avdl", "--generated", "src/b.avdl",
    ]))
    .expect("parses");
    assert_eq!(parsed.schemas, vec![("a.avsc".to_owned(), "src/a.avsc".to_owned())]);
    assert_eq!(parsed.schema_dirs.len(), 1);
    assert_eq!(parsed.generated, vec!["src/b.avdl".to_owned()]);
    assert!(parse(&args(&["--package", "p", "--out", "o"])).is_err());
    assert!(parse(&args(&["--package", "p", "--out", "o", "--schema", "nope"])).is_err());
    assert!(parse(&args(&["--bogus", "x"])).is_err());
    assert!(parse(&args(&["--package"])).is_err());
}
