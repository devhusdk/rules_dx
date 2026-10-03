use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};

const SHARD: &str = "docs/site/demo_extract.ir.textproto";

fn shard() -> String {
    dx_testing::read_runfiles(SHARD)
}

fn header_fields(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.starts_with(' ') && line.contains(": "))
        .map(|line| {
            let (name, value) = line
                .split_once(':')
                .unwrap_or_else(|| panic!("shard line names no field: {line}"));
            (name.to_owned(), value.trim().trim_matches('"').to_owned())
        })
        .collect()
}

fn field(text: &str, name: &str) -> String {
    header_fields(text)
        .into_iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("{SHARD} names no {name} field"))
}

fn symbol_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("  id: "))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .collect()
}

#[test]
fn the_emitted_shard_carries_the_published_schema_version() {
    let text = shard();
    let names: Vec<String> = header_fields(&text)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        names,
        ["schema_major", "schema_minor", "language", "package"],
        "doc_ir.proto numbers the DocIr header fields 1 to 4 in that order"
    );
    assert_eq!(field(&text, "schema_major"), SCHEMA_MAJOR.to_string());
    assert_eq!(field(&text, "schema_minor"), SCHEMA_MINOR.to_string());
}

#[test]
fn every_symbol_id_is_namespaced_and_strictly_increasing() {
    let text = shard();
    let prefix = format!("{}:{}:", field(&text, "language"), field(&text, "package"));
    let ids = symbol_ids(&text);
    assert!(!ids.is_empty(), "{SHARD} carries no symbols");
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("  doc_markdown: "))
            .count(),
        ids.len(),
        "every symbol must carry its doc_markdown"
    );
    for id in &ids {
        assert!(
            id.starts_with(&prefix),
            "symbol id {id} must name its language and package"
        );
    }
    for pair in ids.windows(2) {
        assert!(
            pair[0] < pair[1],
            "symbol ids must strictly increase: {} then {}",
            pair[0],
            pair[1]
        );
    }
}
