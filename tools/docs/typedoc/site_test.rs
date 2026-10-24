//! Asserts the TypeDoc extractor yields real IR and a real rendered, linked, searchable site.

use std::collections::BTreeSet;
use std::path::PathBuf;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use dx_testing::serde_json;

const DIR: &str = "tools/docs/typedoc/fixtures/geometry";

const IDS: [&str; 21] = [
    "Circle",
    "Circle.area",
    "Circle.center",
    "Circle.constructor",
    "Circle.grow",
    "Circle.radius",
    "Paint",
    "Paint.color",
    "Paint.stroke",
    "Palette",
    "Point",
    "Point.x",
    "Point.y",
    "Shape",
    "Shape.area",
    "Shape.center",
    "Unit",
    "Unit.Centimeter",
    "Unit.Millimeter",
    "paint",
    "resize",
];

fn runfiles(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard() -> DocIr {
    let path = runfiles("DX_GEOMETRY_IR");
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    documentation_ir::decode_shard(&bytes).unwrap_or_else(|error| panic!("decode: {error:?}"))
}

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    let id = format!("typescript:geometry:{name}");
    shard
        .symbols
        .iter()
        .find(|symbol| symbol.id == id)
        .unwrap_or_else(|| panic!("no symbol {id}"))
}

fn page() -> String {
    let path = runfiles("DX_GEOMETRY_SITE").join("api.html");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        let tail = &rest[start + open.len()..];
        let Some(end) = tail.find(close) else { break };
        found.push(&tail[..end]);
        rest = &tail[end..];
    }
    found
}

#[test]
fn the_shard_holds_exactly_the_public_api() {
    let shard = shard();
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("typescript", "geometry")
    );
    let ids: Vec<String> = shard.symbols.iter().map(|s| s.id.clone()).collect();
    let expected: Vec<String> = IDS
        .iter()
        .map(|name| format!("typescript:geometry:{name}"))
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn functions_carry_signature_docs_parameters_and_source() {
    let shard = shard();
    let resize = symbol(&shard, "resize");
    assert_eq!(resize.kind, SymbolKind::Function as i32);
    assert_eq!(
        resize.signature_text,
        "function resize(length: number, by: number): number"
    );
    assert_eq!(resize.doc_markdown, "Multiplies a length.");
    let params: Vec<(&str, &str, &str)> = resize
        .params
        .iter()
        .map(|p| (p.name.as_str(), p.r#type.as_str(), p.doc.as_str()))
        .collect();
    assert_eq!(
        params,
        vec![
            ("length", "number", "The length to scale."),
            ("by", "number", "The factor.")
        ]
    );
    let returns = resize.returns.as_ref().expect("a return value");
    assert_eq!(
        (returns.r#type.as_str(), returns.doc.as_str()),
        ("number", "The scaled length.")
    );
    assert_eq!(
        resize.examples,
        vec!["```ts\nscale(2, 3); // 6\n```".to_owned()]
    );
    let source = resize.source.as_ref().expect("a source");
    assert_eq!(source.file, format!("{DIR}/src/units.ts"));
    assert_eq!(source.line, 22);
}

#[test]
fn types_from_the_external_dependency_keep_their_names() {
    let shard = shard();
    assert_eq!(
        symbol(&shard, "paint").signature_text,
        "function paint(color: Property.Color, stroke?: number): Paint"
    );
    assert_eq!(
        symbol(&shard, "Palette").signature_text,
        "const Palette: readonly Property.Color[]"
    );
    assert_eq!(
        symbol(&shard, "Paint.color").signature_text,
        "color: Property.Color"
    );
    assert_eq!(
        symbol(&shard, "Paint.stroke").signature_text,
        "stroke?: number"
    );
}

#[test]
fn classes_enums_and_aliases_map_to_their_kinds() {
    let shard = shard();
    let circle = symbol(&shard, "Circle");
    assert_eq!(circle.kind, SymbolKind::Class as i32);
    assert_eq!(circle.signature_text, "class Circle implements Shape");
    let relations = circle.relations.as_ref().expect("relations");
    assert_eq!(relations.implements, vec!["Shape".to_owned()]);
    assert_eq!(
        symbol(&shard, "Circle.grow").signature_text,
        "grow(factor: number = 2): Circle"
    );
    assert_eq!(
        symbol(&shard, "Circle.constructor").signature_text,
        "constructor(center: Point, radius: number)"
    );
    assert_eq!(symbol(&shard, "Unit").kind, SymbolKind::Enum as i32);
    assert_eq!(
        symbol(&shard, "Unit.Millimeter").signature_text,
        "Millimeter = \"mm\""
    );
    assert_eq!(
        symbol(&shard, "Point").signature_text,
        "type Point = { x: number; y: number }"
    );
    assert_eq!(symbol(&shard, "Shape").kind, SymbolKind::Interface as i32);
}

#[test]
fn every_symbol_names_its_source_and_extraction_configuration() {
    let shard = shard();
    for symbol in &shard.symbols {
        let source = symbol
            .source
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no source", symbol.id));
        assert!(
            source.file.starts_with(&format!("{DIR}/src/")) && source.line > 0,
            "{}: {source:?}",
            symbol.id
        );
        let config = symbol
            .extensions
            .iter()
            .find(|e| e.key == "typescript.config")
            .unwrap_or_else(|| panic!("{} has no configuration identity", symbol.id));
        assert_eq!(
            String::from_utf8_lossy(&config.value),
            "tsconfig=tsconfig.json;entry=src/index.ts;typedoc-json=2.0;options="
        );
    }
}

#[test]
fn private_and_internal_members_stay_out() {
    let ids: BTreeSet<String> = shard().symbols.iter().map(|s| s.id.clone()).collect();
    for hidden in ["Circle.secret", "Circle.debugName"] {
        assert!(
            !ids.contains(&format!("typescript:geometry:{hidden}")),
            "{hidden} leaked"
        );
    }
    let html = page();
    assert!(!html.contains("secret") && !html.contains("debugName"));
}

#[test]
fn the_rendered_page_shows_signatures_docs_and_source_links() {
    let html = page();
    for needle in [
        "<pre><code class=\"language-ts\">function resize(length: number, by: number): number\n</code></pre>",
        "<p>Multiplies a length.</p>",
        "<li><code>length</code>: <code>number</code>. The length to scale.</li>",
        "<strong>Returns</strong>: <code>number</code>. The scaled length.",
        "<pre><code class=\"language-ts\">function paint(color: Property.Color, stroke?: number): Paint\n</code></pre>",
        "<pre><code class=\"language-ts\">class Circle implements Shape\n</code></pre>",
        "<a href=\"https://example.test/geometry/blob/main/tools/docs/typedoc/fixtures/geometry/src/units.ts#L22\">tools/docs/typedoc/fixtures/geometry/src/units.ts:22</a>",
    ] {
        assert!(html.contains(needle), "missing {needle:?}");
    }
}

#[test]
fn every_internal_link_resolves_to_an_anchor() {
    let html = page();
    let anchors: BTreeSet<&str> = between(&html, " id=\"", "\"").into_iter().collect();
    let links: Vec<&str> = between(&html, "href=\"#", "\"");
    assert!(links.len() > 40, "{} links", links.len());
    for link in links {
        assert!(anchors.contains(link), "dangling link #{link}");
    }
    for name in IDS {
        assert!(
            anchors.contains(format!("typescript:geometry:{name}").as_str()),
            "no anchor for {name}"
        );
    }
}

#[test]
fn search_finds_symbols_with_their_docs_and_resolvable_urls() {
    let root = runfiles("DX_GEOMETRY_SITE");
    let raw = std::fs::read_to_string(root.join("searchindex.js")).expect("the search index");
    let body = raw
        .split("JSON.parse('")
        .nth(1)
        .and_then(|rest| rest.rsplit_once("'));"))
        .map(|(json, _)| json.replace("\\'", "'").replace("\\\\", "\\"))
        .expect("an embedded JSON index");
    let index: serde_json::Value = serde_json::from_str(&body).expect("valid index JSON");
    let docs = index["index"]["documentStore"]["docs"]
        .as_object()
        .expect("docs");
    let urls = index["doc_urls"].as_array().expect("urls");
    let html = page();
    let resize = docs
        .values()
        .find(|doc| doc["title"] == "resize")
        .expect("a search record titled resize");
    let text = resize["body"].as_str().expect("body");
    assert!(text.contains("Multiplies a length."), "{text}");
    assert!(
        text.contains("function resize(length: number, by: number): number"),
        "{text}"
    );
    let at = resize["id"]
        .as_str()
        .and_then(|id| id.parse::<usize>().ok())
        .expect("id");
    let url = urls[at].as_str().expect("url");
    let fragment = url.strip_prefix("api.html#").expect("an api.html url");
    assert!(
        html.contains(&format!(" id=\"{fragment}\"")),
        "{url} does not resolve"
    );
    for doc in docs.values() {
        let at: usize = doc["id"].as_str().expect("id").parse().expect("numeric id");
        let fragment = urls[at]
            .as_str()
            .expect("url")
            .strip_prefix("api.html#")
            .expect("api url");
        assert!(
            html.contains(&format!(" id=\"{fragment}\"")),
            "dangling search url {fragment}"
        );
    }
}
