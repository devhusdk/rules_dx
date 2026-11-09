use super::*;
use serde_json::json;

fn page(preamble: &str, content: &str) -> String {
    json!({"preamble": preamble, "content": content}).to_string()
}

const CONTENT: &str = r##"<div class="odoc-spec"><div class="spec type anchored" id="type-t"><a href="#type-t" class="anchor"></a><code><span><span class="keyword">type</span> t</span><span> = </span><span>{</span></code><ol><li id="type-t.radius" class="def record field anchored"><code><span>radius : float;</span></code><div class="def-doc"><span class="comment-delim">(*</span><p>The radius.</p><span class="comment-delim">*)</span></div></li></ol><code><span>}</span></code></div><div class="spec-doc"><p>A circle.</p></div></div><div class="odoc-spec"><div class="spec value anchored" id="val-area"><a href="#val-area" class="anchor"></a><code><span><span class="keyword">val</span> area : <span><a href="#type-t">t</a> <span class="arrow">&#45;&gt;</span></span> float</span></code></div><div class="spec-doc"><p>Area of a <code>t</code> &amp; more.</p></div></div><div class="odoc-spec"><div class="spec value anchored" id="val-t"><code><span>val t : int</span></code></div></div>"##;

fn pages() -> Vec<(String, String)> {
    vec![
        (
            "geo/Shapes".to_owned(),
            page("<p>Shapes doc.</p>", CONTENT),
        ),
        (
            "geo/Shapes/Sub".to_owned(),
            page(
                "",
                r#"<div class="odoc-spec"><div class="spec value anchored" id="val-inner"><code><span>val inner : int</span></code></div><div class="spec-doc"><p>Inner.</p></div></div>"#,
            ),
        ),
        (
            "geo/Shapes/module-type-S".to_owned(),
            page("<p>A signature.</p>", ""),
        ),
    ]
}

fn find<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == format!("ocaml:geo:{id}"))
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn converts_pages_and_specs() {
    let shard = convert(&pages(), "geo", "").unwrap();
    assert_eq!(shard.language, "ocaml");
    let module = find(&shard, "Shapes");
    assert_eq!(module.kind, SymbolKind::Module as i32);
    assert_eq!(module.doc_markdown, "Shapes doc.");
    assert_eq!(module.signature_text, "module Shapes");
    let area = find(&shard, "Shapes.area");
    assert_eq!(area.kind, SymbolKind::Function as i32);
    assert_eq!(area.signature_text, "val area : t -> float");
    assert_eq!(area.doc_markdown, "Area of a `t` & more.");
    assert_eq!(
        area.relations.as_ref().unwrap().member_of,
        vec!["ocaml:geo:Shapes".to_owned()]
    );
    let record = find(&shard, "Shapes.t");
    assert_eq!(record.signature_text, "type t = { radius : float; }");
    assert_eq!(record.doc_markdown, "A circle.");
}

#[test]
fn nested_and_signature_pages_get_their_own_symbols() {
    let shard = convert(&pages(), "geo", "").unwrap();
    assert_eq!(
        find(&shard, "Shapes.Sub.inner")
            .relations
            .as_ref()
            .unwrap()
            .member_of,
        vec!["ocaml:geo:Shapes.Sub".to_owned()]
    );
    let signature = find(&shard, "Shapes.S");
    assert_eq!(signature.kind, SymbolKind::Interface as i32);
    assert_eq!(signature.signature_text, "module type S");
}

#[test]
fn colliding_names_stay_unique_and_undocumented_items_are_incomplete() {
    let shard = convert(&pages(), "geo", "").unwrap();
    let value = find(&shard, "Shapes.t~2Dval");
    assert_eq!(value.kind, SymbolKind::Function as i32);
    assert!(value.extensions.iter().any(|e| e.key == KEY_INCOMPLETE));
}

#[test]
fn rejects_bad_input() {
    assert!(convert(&pages(), "a b", "").unwrap_err().contains("invalid package"));
    assert!(convert(&[], "geo", "").unwrap_err().contains("no public"));
    let bad = vec![("geo/M".to_owned(), "{".to_owned())];
    assert!(convert(&bad, "geo", "").unwrap_err().contains("malformed"));
    let flat = vec![("M".to_owned(), page("", ""))];
    assert!(convert(&flat, "geo", "").unwrap_err().contains("package directory"));
}

#[test]
fn html_reader_decodes_and_renders() {
    let nodes = html::parse("<ul><li>one &#45; <em>two</em></li></ul><pre>a &lt; b</pre>");
    let md = markdown(&Node::root(nodes));
    assert!(md.contains("- one - *two*"), "{md}");
    assert!(md.contains("```ocaml\na < b\n```"), "{md}");
}

#[test]
fn parse_args_requires_every_flag_once() {
    let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let ok = parse_args(&args(&["--html", "a", "--package", "p", "--out", "o"])).unwrap();
    assert_eq!(ok.package, "p");
    assert!(parse_args(&args(&["--html", "a"])).unwrap_err().contains("required"));
    assert!(parse_args(&args(&["--bogus", "a"])).unwrap_err().contains("unknown"));
    assert!(parse_args(&args(&["--html"])).unwrap_err().contains("needs a value"));
    assert_eq!(
        parse_args(&args(&["--html", "a", "--html", "b", "--package", "p", "--out", "o"]))
            .unwrap()
            .html,
        vec!["a".to_owned(), "b".to_owned()]
    );
    assert!(
        parse_args(&args(&["--package", "a", "--package", "b"]))
            .unwrap_err()
            .contains("more than once")
    );
    assert!(parse_args(&args(&["--html", ""])).unwrap_err().contains("empty"));
}

#[test]
fn config_is_recorded_on_every_symbol() {
    let shard = convert(&pages(), "geo", "odoc=3.2.2").unwrap();
    assert!(!shard.symbols.is_empty());
    for symbol in &shard.symbols {
        let found = symbol.extensions.iter().find(|e| e.key == KEY_CONFIG).unwrap();
        assert_eq!(found.value, b"odoc=3.2.2");
    }
    let bare = convert(&pages(), "geo", "").unwrap();
    assert!(bare.symbols.iter().all(|s| s.extensions.iter().all(|e| e.key != KEY_CONFIG)));
}

#[test]
fn parses_config_flag() {
    let args: Vec<String> = ["--html", "h", "--package", "p", "--config", "c", "--out", "o"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    assert_eq!(parse_args(&args).unwrap().config, "c");
}
