use super::*;
use serde_json::json;

const GOOGLE: &str = include_str!("../testdata/mylib.google.json");
const NUMPY: &str = include_str!("../testdata/npy.numpy.json");
const P: &str = "python:mylib:";

fn google() -> DocIr {
    convert(GOOGLE, "mylib").unwrap()
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let prefix = format!("python:{}:", shard.package);
    let id = format!("{prefix}{qualified}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing: {:?}", ids(shard)))
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

fn incomplete(symbol: &Symbol) -> String {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == KEY_INCOMPLETE)
        .map(|e| String::from_utf8(e.value.clone()).unwrap())
        .unwrap_or_default()
}

fn edited(edit: impl FnOnce(&mut Value)) -> String {
    let mut document: Value = serde_json::from_str(GOOGLE).unwrap();
    edit(&mut document);
    document.to_string()
}

#[test]
fn ids_are_sorted_and_private_names_are_absent() {
    let shard = google();
    let all = ids(&shard);
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted);
    assert_eq!(
        all,
        vec![
            "python:mylib:Alias",
            "python:mylib:Color",
            "python:mylib:Color.GREEN",
            "python:mylib:Color.RED",
            "python:mylib:Helper",
            "python:mylib:LIMIT",
            "python:mylib:Shape",
            "python:mylib:Shape.grow",
            "python:mylib:Shape.label",
            "python:mylib:Shape.scale",
            "python:mylib:__init__",
            "python:mylib:area",
            "python:mylib:shapes",
            "python:mylib:shapes.Circle",
            "python:mylib:shapes.Circle.load",
            "python:mylib:shapes.Circle.unit",
            "python:mylib:shapes.helper",
        ]
    );
}

#[test]
fn kinds_cover_every_shape() {
    let shard = google();
    let kind = |q: &str| symbol(&shard, q).kind;
    assert_eq!(kind("__init__"), SymbolKind::Module as i32);
    assert_eq!(kind("shapes"), SymbolKind::Module as i32);
    assert_eq!(kind("Shape"), SymbolKind::Class as i32);
    assert_eq!(kind("Color"), SymbolKind::Enum as i32);
    assert_eq!(kind("Color.RED"), SymbolKind::EnumVariant as i32);
    assert_eq!(kind("area"), SymbolKind::Function as i32);
    assert_eq!(kind("Shape.grow"), SymbolKind::Method as i32);
    assert_eq!(kind("Shape.label"), SymbolKind::Property as i32);
    assert_eq!(kind("Shape.scale"), SymbolKind::Field as i32);
    assert_eq!(kind("LIMIT"), SymbolKind::Constant as i32);
    assert_eq!(kind("Alias"), SymbolKind::TypeAlias as i32);
}

#[test]
fn signatures_render_parameters_returns_and_bases() {
    let shard = google();
    let sig = |q: &str| symbol(&shard, q).signature_text.clone();
    assert_eq!(sig("area"), "def area(width: int, height: int = 1) -> int");
    assert_eq!(sig("Shape"), "class Shape(Helper)");
    assert_eq!(sig("Shape.grow"), "def grow(factor: float = 2.0) -> Shape");
    assert_eq!(sig("Shape.label"), "label: str");
    assert_eq!(sig("Shape.scale"), "scale: float = 1.0");
    assert_eq!(sig("LIMIT"), "LIMIT: int = 10");
    assert_eq!(sig("Alias"), "type Alias = list[int]");
    assert_eq!(sig("Color.RED"), "RED = 1");
    assert_eq!(
        sig("shapes.Circle.load"),
        "async def load(path: str, /, *names: str, strict: bool = False, **extra: int) -> Circle"
    );
    assert_eq!(sig("shapes.Circle.unit"), "def unit() -> Circle");
}

#[test]
fn docs_params_returns_and_examples_come_from_sections() {
    let shard = google();
    let grow = symbol(&shard, "Shape.grow");
    assert_eq!(grow.doc_markdown, "Grow it.");
    assert_eq!(grow.params.len(), 1);
    assert_eq!(grow.params[0].name, "factor");
    assert_eq!(grow.params[0].r#type, "float");
    assert_eq!(grow.params[0].doc, "The multiplier.");
    let returns = grow.returns.as_ref().unwrap();
    assert_eq!(
        (returns.r#type.as_str(), returns.doc.as_str()),
        ("Shape", "The grown shape.")
    );
    assert_eq!(grow.examples, vec![">>> Shape(\"a\").grow()"]);
    assert_eq!(incomplete(grow), "");
    let class = symbol(&shard, "Shape");
    assert_eq!(class.params[0].name, "name");
    assert_eq!(class.params[1].name, "sides");
    assert_eq!(class.params[0].doc, "The shape name.");
}

#[test]
fn numpy_docstyle_supplies_types_and_docs() {
    let shard = convert(NUMPY, "npy").unwrap();
    let scale = symbol(&shard, "scale");
    assert_eq!(scale.signature_text, "def scale(values, factor=2)");
    assert_eq!(scale.params[0].r#type, "list of int");
    assert_eq!(scale.params[1].doc, "The factor.");
    assert_eq!(scale.returns.as_ref().unwrap().r#type, "list of int");
    assert_eq!(incomplete(scale), "");
}

#[test]
fn reexports_use_the_public_definition_as_canonical() {
    let shard = google();
    let circle = symbol(&shard, "shapes.Circle");
    let relations = circle.relations.as_ref().unwrap();
    assert_eq!(relations.reexported_as, vec![format!("{P}Circle")]);
    assert_eq!(relations.member_of, vec![format!("{P}shapes")]);
    assert_eq!(circle.display_name, "mylib.shapes.Circle");
}

#[test]
fn reexports_of_private_definitions_use_the_first_public_path() {
    let shard = google();
    let area = symbol(&shard, "area");
    assert_eq!(area.display_name, "mylib.area");
    assert_eq!(
        area.relations.as_ref().unwrap().member_of,
        vec![format!("{P}__init__")]
    );
    assert!(area.relations.as_ref().unwrap().reexported_as.is_empty());
    assert_eq!(symbol(&shard, "Helper").display_name, "mylib.Helper");
    assert_eq!(symbol(&shard, "__init__").display_name, "mylib");
    assert!(shard.symbols.iter().all(|s| !s.id.contains("_impl")));
    assert!(shard.symbols.iter().all(|s| !s.id.contains("_priv")));
    assert!(shard.symbols.iter().all(|s| !s.id.contains("_hidden")));
}

#[test]
fn incomplete_reasons_name_each_gap() {
    let shard = google();
    assert_eq!(
        incomplete(symbol(&shard, "shapes.helper")),
        "doc;return_type"
    );
    assert_eq!(
        incomplete(symbol(&shard, "shapes.Circle.load")),
        "params=path,names,strict,extra"
    );
    assert_eq!(incomplete(symbol(&shard, "Shape.label")), "");
}

#[test]
fn sources_and_extensions_are_recorded() {
    let shard = google();
    let grow = symbol(&shard, "Shape.grow");
    let source = grow.source.as_ref().unwrap();
    assert_eq!(source.file, "mylib/__init__.py");
    assert_eq!(source.line, 42);
    assert!(!source.generated);
    let config = String::from_utf8(grow.extensions[0].value.clone()).unwrap();
    assert!(
        config.starts_with("docstyle=google;format=1;griffe=2.4.1;python="),
        "{config}"
    );
    assert_eq!(grow.extensions[0].key, KEY_CONFIG);
}

#[test]
fn encoding_is_stable() {
    let shard = google();
    let bytes = documentation_ir::encode_shard(&shard).unwrap();
    let again = documentation_ir::encode_shard(&documentation_ir::decode_shard(&bytes).unwrap());
    assert_eq!(bytes, again.unwrap());
    assert_eq!(bytes, documentation_ir::encode_shard(&google()).unwrap());
}

#[test]
fn ids_escape_unusual_characters() {
    assert_eq!(escape_segment("Plain_1"), "Plain_1");
    assert_eq!(escape_segment("a-b"), "a~2Db");
    assert_eq!(escape_segment("é"), "~C3~A9");
}

#[test]
fn failures_are_rejected() {
    assert!(convert("{", "mylib").unwrap_err().contains("malformed"));
    assert!(convert(GOOGLE, "other")
        .unwrap_err()
        .contains("describes package"));
    assert!(convert(GOOGLE, "a.b")
        .unwrap_err()
        .contains("invalid package"));
    let format = edited(|d| d["format"] = json!(2));
    assert!(convert(&format, "mylib")
        .unwrap_err()
        .contains("not supported"));
    let module = edited(|d| d["module"]["path"] = json!("elsewhere"));
    assert!(convert(&module, "mylib")
        .unwrap_err()
        .contains("is not package"));
    let empty = edited(|d| d["module"]["members"] = json!({}));
    assert!(convert(&empty, "mylib")
        .unwrap_err()
        .contains("no public Python API"));
    let private = edited(|d| {
        d["module"]["exports"] = json!([]);
        d["module"]["members"]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| name.starts_with('_'));
    });
    assert!(convert(&private, "mylib")
        .unwrap_err()
        .contains("no public Python API"));
    let unsafe_path = edited(|d| {
        d["module"]["members"]["LIMIT"]["relative_package_filepath"] = json!("../escape.py");
    });
    assert!(convert(&unsafe_path, "mylib")
        .unwrap_err()
        .contains("invalid IR"));
}

#[test]
fn parse_requires_every_flag() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(parse(&args(&[
        "--json",
        "a",
        "--package",
        "p",
        "--root",
        "r",
        "--srcs",
        "s",
        "--out",
        "o"
    ]))
    .is_ok());
    assert!(parse(&args(&["--json", "a"])).is_err());
    assert!(parse(&args(&["--bad", "a"])).is_err());
    assert!(parse(&args(&["--json", "a", "--json", "b"])).is_err());
}

#[test]
fn bare_star_is_kept_before_leading_keyword_only_parameters() {
    let keyword = json!({"name": "a", "kind": "keyword-only"});
    let this = json!({"name": "self", "kind": "positional or keyword"});
    assert_eq!(render_parameters(&[keyword.clone()], false), "*, a");
    assert_eq!(render_parameters(&[this, keyword.clone()], true), "*, a");
    let star = json!({"name": "args", "kind": "variadic positional"});
    assert_eq!(render_parameters(&[star, keyword], false), "*args, a");
}

#[test]
fn sources_are_mapped_through_the_manifest() {
    let manifest = "bin/p.pyroot/mylib/__init__.py\tpkg/src/mylib/__init__.py\t0\n";
    let sources = parse_sources(manifest).unwrap();
    let mapped = convert_mapped(GOOGLE, "mylib", &|file| {
        sources
            .get(&format!("bin/p.pyroot/{file}"))
            .cloned()
            .ok_or_else(|| format!("unmapped {file}"))
    });
    assert!(mapped.unwrap_err().starts_with("unmapped "));
    let generated = convert_mapped(GOOGLE, "mylib", &|file| {
        Ok((format!("pkg/src/{file}"), file.ends_with("x.py")))
    })
    .unwrap();
    for symbol in &generated.symbols {
        let source = symbol.source.as_ref().unwrap();
        assert!(source.file.starts_with("pkg/src/mylib/"), "{}", source.file);
        assert_eq!(source.generated, source.file.ends_with("x.py"));
    }
}

#[test]
fn malformed_manifest_lines_are_rejected() {
    assert!(parse_sources("a\tb\t0\n").is_ok());
    assert!(parse_sources("a\tb\n").is_err());
    assert!(parse_sources("a\tb\t2\n").is_err());
    assert!(parse_sources("a\t../b\t0\n").is_err());
    assert!(parse_sources("a\t/b\t0\n").is_err());
}
