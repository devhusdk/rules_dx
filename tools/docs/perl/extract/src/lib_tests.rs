use super::*;
use serde_json::json;

fn object(kind: &str, path: &str, name: &str, namespace: &str, visibility: &str) -> Value {
    json!({
        "type": kind, "path": path, "name": name, "namespace": namespace,
        "signature": format!("sub {name}"), "docstring": format!("Doc of {name}."),
        "file": "lib/Geo.pm", "generated": false, "line": 3, "visibility": visibility
    })
}

fn document(objects: Vec<Value>) -> String {
    json!({"format": 1, "perl": "5.40.1", "objects": Value::Array(objects)}).to_string()
}

#[test]
fn packages_and_subs_become_symbols() {
    let shard = convert(
        &document(vec![
            object("package", "Geo", "Geo", "", "public"),
            object("sub", "Geo::area", "area", "Geo", "public"),
        ]),
        "geo",
    )
    .unwrap();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["perl:geo:Geo", "perl:geo:Geo.area"]);
    assert_eq!(shard.symbols[0].kind, SymbolKind::Module as i32);
    assert_eq!(shard.symbols[1].kind, SymbolKind::Function as i32);
    let relations = shard.symbols[1].relations.as_ref().unwrap();
    assert_eq!(relations.member_of, ["perl:geo:Geo"]);
}

#[test]
fn private_subs_are_skipped() {
    let shard = convert(
        &document(vec![
            object("package", "Geo", "Geo", "", "public"),
            object("sub", "Geo::_hidden", "_hidden", "Geo", "private"),
        ]),
        "geo",
    )
    .unwrap();
    assert_eq!(shard.symbols.len(), 1);
}

#[test]
fn missing_docs_are_marked_incomplete() {
    let mut bare = object("package", "Geo", "Geo", "", "public");
    bare["docstring"] = json!("");
    let shard = convert(&document(vec![bare]), "geo").unwrap();
    assert!(
        shard.symbols[0]
            .extensions
            .iter()
            .any(|e| e.key == KEY_INCOMPLETE && e.value == b"doc")
    );
}

#[test]
fn rejects_bad_input() {
    let error = convert("{", "geo").unwrap_err();
    assert!(matches!(error, PerlExtractError::Malformed(_)));
    assert!(error.to_string().contains("malformed"));
    let error = convert(&document(vec![]), "geo").unwrap_err();
    assert!(matches!(error, PerlExtractError::NoPublicApi { .. }));
    assert_eq!(
        error.to_string(),
        "POD JSON lists no public Perl API of package 'geo'"
    );
    let error = convert(&document(vec![]), "a:b").unwrap_err();
    assert!(matches!(error, PerlExtractError::InvalidPackage { .. }));
    assert_eq!(error.to_string(), "invalid package 'a:b'");
    let unknown = object("glob", "Geo", "Geo", "", "public");
    let error = convert(&document(vec![unknown]), "geo").unwrap_err();
    assert!(matches!(error, PerlExtractError::UnknownType { .. }));
    assert_eq!(
        error.to_string(),
        "POD JSON object 0 has unknown type 'glob'"
    );
}

#[test]
fn run_reports_an_unreadable_input() {
    let options = Options {
        json: "/nonexistent/perl.json".to_owned(),
        package: "geo".to_owned(),
        out: "out".to_owned(),
    };
    assert!(matches!(run(&options), Err(PerlExtractError::Read(_))));
}

#[test]
fn parse_reads_every_flag() {
    let args: Vec<String> = ["--json", "a", "--package", "p", "--out", "o"]
        .map(String::from)
        .to_vec();
    assert_eq!(parse(&args).unwrap().package, "p");
}
