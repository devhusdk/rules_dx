use documentation_ir::{decode_shard_text, encode_shard, proto, SCHEMA_MAJOR, SCHEMA_MINOR};
use proto::{DocIr, Symbol};

const SHARD: &str = "docs/site/demo_extract.ir.textproto";

const ID_NAMES: [&str; 6] = [
    "AccountService",
    "AccountService.create",
    "AccountService.escape",
    "AccountService.get",
    "AccountService.quote",
    "AccountService.union",
];

const DOCS: [&str; 6] = [
    "Creates and reads accounts.",
    "Creates a new account.",
    r"Reads a path like C:\temp.",
    "Fetches an account by ID.",
    r#"Accepts a "quoted" name."#,
    "Reads a Result<A | B, Error>.",
];

fn shard() -> String {
    dx_testing::read_runfiles(SHARD)
}

fn decoded() -> DocIr {
    decode_shard_text(&shard()).unwrap_or_else(|error| panic!("{SHARD} does not decode: {error:?}"))
}

fn encoded(shard: &DocIr) -> Vec<u8> {
    encode_shard(shard).unwrap_or_else(|error| panic!("shard does not encode: {error:?}"))
}

fn expected() -> DocIr {
    DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "python".to_owned(),
        package: "demo".to_owned(),
        symbols: ID_NAMES
            .iter()
            .zip(DOCS)
            .map(|(name, doc)| Symbol {
                id: format!("python:demo:{name}"),
                doc_markdown: doc.to_owned(),
                ..Symbol::default()
            })
            .collect(),
    }
}

#[test]
fn the_emitted_shard_decodes_to_the_message_the_binary_codec_encodes() {
    let decoded = decoded();
    assert_eq!(decoded, expected());
    assert_eq!(
        encoded(&decoded),
        encoded(&expected()),
        "doc_ir.proto is the spec of the shard docs_extract emits"
    );
    assert_eq!(decoded.schema_major, SCHEMA_MAJOR);
    assert_eq!(decoded.schema_minor, SCHEMA_MINOR);
}

#[test]
fn the_emitted_header_numbers_the_proto_fields_in_order() {
    let text = shard();
    let names: Vec<&str> = text
        .lines()
        .take(4)
        .map(|line| line.split(':').next().unwrap_or_default())
        .collect();
    assert_eq!(
        names,
        ["schema_major", "schema_minor", "language", "package"],
        "doc_ir.proto numbers the DocIr header fields 1 to 4 in that order"
    );
}

#[test]
fn every_symbol_is_namespaced_and_documented() {
    let decoded = decoded();
    assert!(!decoded.symbols.is_empty(), "{SHARD} carries no symbols");
    for symbol in &decoded.symbols {
        assert!(
            symbol
                .id
                .starts_with(&format!("{}:{}:", decoded.language, decoded.package)),
            "symbol id {} must name its language and package",
            symbol.id
        );
        assert!(
            !symbol.doc_markdown.is_empty(),
            "symbol {} lost its doc_markdown",
            symbol.id
        );
    }
}

#[test]
fn doc_markdown_is_escaped_for_textproto_and_decodes_whole() {
    let text = shard();
    let raw: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  doc_markdown: "))
        .collect();
    assert_eq!(
        raw,
        [
            r#""Creates and reads accounts.""#,
            r#""Creates a new account.""#,
            r#""Reads a path like C:\\temp.""#,
            r#""Fetches an account by ID.""#,
            r#""Accepts a \"quoted\" name.""#,
            r#""Reads a Result<A | B, Error>.""#,
        ],
        "doc_ir.proto types doc_markdown as a string, so a quote and a backslash must be escaped"
    );
    let decoded = decode_shard_text(&text)
        .unwrap_or_else(|error| panic!("{SHARD} does not decode: {error:?}"));
    let docs: Vec<&str> = decoded
        .symbols
        .iter()
        .map(|symbol| symbol.doc_markdown.as_str())
        .collect();
    assert_eq!(docs, DOCS, "escaping must be lossless");
}
