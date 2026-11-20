use super::*;

const JSON: &str = r#"{"format":1,"language":"elixir","tool":"Code.fetch_docs","version":"1.20.4","symbols":[
{"kind":"module","module":"Geo.Shape","name":"","arity":0,"signature":"defmodule Geo.Shape","params":[],"doc":"Shapes.\n","file":"a/shape.ex","line":1},
{"kind":"function","module":"Geo.Shape","name":"area","arity":1,"signature":"area(shape)","params":["shape"],"doc":"","file":"a/shape.ex","line":5},
{"kind":"macro","module":"Geo.Shape","name":"empty?","arity":0,"signature":"empty?()","params":[],"doc":"d","file":"a/shape.ex","line":9}]}"#;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn converts_modules_and_members() {
    let shard = convert(JSON, "geo").expect("converts");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "elixir:geo:Geo.Shape",
            "elixir:geo:Geo.Shape.area@function-1",
            "elixir:geo:Geo.Shape.empty~3F@macro-0",
        ]
    );
    assert_eq!(shard.symbols[0].doc_markdown, "Shapes.");
    assert_eq!(shard.symbols[1].params[0].name, "shape");
    let incomplete = shard.symbols[1]
        .extensions
        .iter()
        .any(|e| e.key == KEY_INCOMPLETE);
    assert!(incomplete);
}

#[test]
fn rejects_unknown_language_and_empty_symbols() {
    let bad = JSON.replace("\"elixir\"", "\"cobol\"");
    let error = convert(&bad, "geo").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::UnknownLanguage { .. }));
    assert_eq!(
        error.to_string(),
        "driver JSON has unknown language 'cobol'"
    );
    let empty = r#"{"format":1,"language":"erlang","tool":"edoc","version":"1","symbols":[]}"#;
    let error = convert(empty, "geo").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::NoPublicApi { .. }));
    assert_eq!(
        error.to_string(),
        "driver JSON lists no public erlang API of package 'geo'"
    );
}

#[test]
fn rejects_malformed_documents() {
    let error = convert("{", "geo").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::Malformed(_)));
    assert_eq!(
        error.to_string(),
        "driver JSON is malformed: EOF while parsing an object at line 1 column 1"
    );
    let symbols = r#"{"format":1,"language":"erlang","tool":"edoc","version":"1","symbols":{}}"#;
    let error = convert(symbols, "geo").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::Json(_)));
    assert_eq!(error.to_string(), "driver JSON 'symbols' is not a list");
    let kind = JSON.replace("\"macro\"", "\"record\"");
    let error = convert(&kind, "geo").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::UnknownKind { .. }));
    assert_eq!(
        error.to_string(),
        "driver JSON symbol 2 has unknown kind 'record'"
    );
    let error = convert(JSON, "a:b").expect_err("rejected");
    assert!(matches!(error, BeamExtractError::InvalidPackage { .. }));
    assert_eq!(error.to_string(), "invalid package 'a:b'");
}

#[test]
fn run_reports_an_unreadable_input() {
    let options = Options {
        json: "/nonexistent/beam.json".to_owned(),
        package: "geo".to_owned(),
        out: "out".to_owned(),
    };
    let error = run(&options).expect_err("rejected");
    assert!(matches!(error, BeamExtractError::Read(_)));
    assert!(
        error
            .to_string()
            .starts_with("cannot read /nonexistent/beam.json: "),
        "{error}"
    );
}

#[test]
fn parses_flags() {
    let options = parse(&args(&["--json", "a", "--package", "p", "--out", "o"])).expect("parses");
    assert_eq!(options.package, "p");
}
