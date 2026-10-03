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

fn doc_markdowns(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("  doc_markdown: "))
        .collect()
}

fn unescape_textproto(raw: &str) -> Result<String, String> {
    let body = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(|| format!("{raw} is not a closed quoted textproto string"))?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            if ch == '"' {
                return Err(format!("{raw} leaves a quote unescaped"));
            }
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => return Err(format!("{raw} carries the invalid escape \\{other}")),
            None => return Err(format!("{raw} ends in a dangling escape")),
        }
    }
    Ok(out)
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

#[test]
fn doc_markdown_is_escaped_for_textproto_and_round_trips() {
    let text = shard();
    let raw = doc_markdowns(&text);
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
    assert_eq!(
        raw.iter()
            .copied()
            .map(unescape_textproto)
            .collect::<Result<Vec<_>, _>>(),
        Ok(vec![
            "Creates and reads accounts.".to_owned(),
            "Creates a new account.".to_owned(),
            r"Reads a path like C:\temp.".to_owned(),
            "Fetches an account by ID.".to_owned(),
            r#"Accepts a "quoted" name."#.to_owned(),
            "Reads a Result<A | B, Error>.".to_owned(),
        ]),
        "escaping must be lossless, so the shard decodes back to the fixture prose"
    );
}

#[test]
fn the_textproto_reader_rejects_what_the_emitter_must_never_write() {
    assert_eq!(
        unescape_textproto(r#""plain text""#),
        Ok("plain text".to_owned())
    );
    assert_eq!(
        unescape_textproto(r#""a \"quoted\" name""#),
        Ok(r#"a "quoted" name"#.to_owned())
    );
    assert_eq!(
        unescape_textproto(r#""C:\\temp""#),
        Ok(r"C:\temp".to_owned())
    );
    for bad in [
        r#""a "quoted" name""#,
        r#""C:\temp""#,
        r#""dangling\"#,
        r#""unterminated"#,
        r#"not quoted"#,
    ] {
        assert!(
            unescape_textproto(bad).is_err(),
            "{bad} is malformed textproto and must not decode"
        );
    }
}
