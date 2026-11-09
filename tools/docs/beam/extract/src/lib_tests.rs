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
    assert!(convert(&bad, "geo").is_err());
    let empty = r#"{"format":1,"language":"erlang","tool":"edoc","version":"1","symbols":[]}"#;
    assert!(convert(empty, "geo").is_err());
}

#[test]
fn parses_flags() {
    let options = parse(&args(&["--json", "a", "--package", "p", "--out", "o"])).expect("parses");
    assert_eq!(options.package, "p");
    assert!(parse(&args(&["--json", "a"])).is_err());
    assert!(parse(&args(&["--bogus", "a"])).is_err());
}
