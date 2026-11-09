use super::*;

const DUMP: &str = r#"{"schema":1,"files":[
 {"path":"src/geometry.zig","doc":"Geometry helpers.","imports":["shapes.zig"],"fields":[],"decls":[
  {"name":"shapes","public":true,"kind":"import","doc":"","line":2,"signature":"pub const shapes = @import(\"shapes.zig\")","import":"shapes.zig"},
  {"name":"units","public":true,"kind":"import","doc":"Unit helpers.","line":3,"signature":"pub const units = @import(\"units\")","import":"units"},
  {"name":"std","public":false,"kind":"import","doc":"","line":1,"signature":"const std = @import(\"std\")","import":"std"},
  {"name":"Circle","public":true,"kind":"struct","doc":"A circle.","line":4,"signature":"pub const Circle = struct",
   "fields":[{"name":"radius","type":"f64","doc":"Radius.","line":5}],
   "members":[{"name":"area","public":true,"kind":"fn","doc":"Area.","line":6,"signature":"pub fn area(self: Circle) f64","params":[{"name":"self","type":"Circle"}],"returns":"f64"},
              {"name":"secret","public":false,"kind":"fn","doc":"","line":7,"signature":"fn secret() void"}]},
  {"name":"add","public":true,"kind":"fn","doc":"Adds.","line":9,"signature":"pub fn add(a: i32, b: i32) i32","params":[{"name":"a","type":"i32"},{"name":"b","type":"i32"}],"returns":"i32"},
  {"name":"hidden","public":false,"kind":"fn","doc":"","line":10,"signature":"fn hidden() void"},
  {"name":"Color","public":true,"kind":"enum","doc":"","line":11,"signature":"pub const Color = enum","fields":[{"name":"red","type":"","doc":"Red.","line":12}]}]},
 {"path":"src/shapes.zig","doc":"Shapes.","imports":[],"fields":[],"decls":[
  {"name":"unit","public":true,"kind":"const","doc":"One.","line":3,"signature":"pub const unit: f64 = 1.0"},
  {"name":"off","public":false,"kind":"const","doc":"","line":4,"signature":"const off = 0"}]},
 {"path":"src/unused.zig","doc":"","imports":[],"fields":[],"decls":[
  {"name":"nope","public":true,"kind":"fn","doc":"","line":1,"signature":"pub fn nope() void"}]}]}"#;

fn sources() -> Vec<Source> {
    ["geometry", "shapes", "unused"]
        .iter()
        .map(|name| Source {
            exec_path: format!("src/{name}.zig"),
            rel: format!("lib/src/{name}.zig"),
            generated: false,
        })
        .collect()
}

fn shard(config: &str) -> DocIr {
    convert(DUMP, &sources(), "geometry", config).expect("converts")
}

#[test]
fn private_declarations_and_unreachable_files_are_dropped() {
    let shard = shard("");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "zig:geometry:geometry",
            "zig:geometry:geometry.Circle",
            "zig:geometry:geometry.Circle.area",
            "zig:geometry:geometry.Circle.radius",
            "zig:geometry:geometry.Color",
            "zig:geometry:geometry.Color.red",
            "zig:geometry:geometry.add",
            "zig:geometry:geometry.shapes",
            "zig:geometry:geometry.shapes.unit",
            "zig:geometry:geometry.units",
        ]
    );
}

#[test]
fn reexport_becomes_a_module_with_its_own_source() {
    let shard = shard("x");
    let module = shard
        .symbols
        .iter()
        .find(|s| s.id == "zig:geometry:geometry.shapes")
        .expect("module");
    assert_eq!(module.kind, SymbolKind::Module as i32);
    assert_eq!(module.doc_markdown, "Shapes.");
    assert_eq!(
        module.source.as_ref().expect("source").file,
        "lib/src/shapes.zig"
    );
    assert_eq!(
        module.extensions[0].value,
        b"zig-ast=1;package=geometry;config=x"
    );
}

#[test]
fn params_returns_and_kinds() {
    let shard = shard("");
    let by_id = |id: &str| {
        shard
            .symbols
            .iter()
            .find(|s| s.id == format!("zig:geometry:{id}"))
            .expect("symbol")
    };
    let add = by_id("geometry.add");
    assert_eq!(add.signature_text, "pub fn add(a: i32, b: i32) i32");
    assert_eq!(add.params[1].name, "b");
    assert_eq!(add.params[1].r#type, "i32");
    assert_eq!(add.returns.as_ref().expect("returns").r#type, "i32");
    assert_eq!(
        by_id("geometry.Circle.area").kind,
        SymbolKind::Method as i32
    );
    assert_eq!(by_id("geometry.Circle").kind, SymbolKind::Class as i32);
    assert_eq!(
        by_id("geometry.Color.red").kind,
        SymbolKind::EnumVariant as i32
    );
    assert_eq!(
        by_id("geometry.Circle.radius").signature_text,
        "radius: f64"
    );
    assert_eq!(
        by_id("geometry.add").source.as_ref().expect("source").line,
        9
    );
}

#[test]
fn unknown_file_and_schema_are_rejected() {
    let error = convert(DUMP, &[], "geometry", "").expect_err("rejected");
    assert!(error.contains("not in the srcs manifest"), "{error}");
    let error = convert(r#"{"schema":2,"files":[]}"#, &[], "g", "").expect_err("rejected");
    assert!(error.contains("schema"), "{error}");
}

#[test]
fn parse_rejects_bad_arguments() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(parse(&args(&["--nope", "x"])).is_err());
    assert!(parse(&args(&["--dump", "a"])).is_err());
    let all = args(&[
        "--dump",
        "a",
        "--sources",
        "s",
        "--out",
        "o",
        "--package",
        "p",
    ]);
    assert_eq!(parse(&all).expect("parses").package, "p");
}

#[test]
fn resolve_folds_relative_segments() {
    assert_eq!(resolve("src/a.zig", "b.zig"), "src/b.zig");
    assert_eq!(resolve("src/a/x.zig", "../b.zig"), "src/b.zig");
    assert_eq!(resolve("a.zig", "./b.zig"), "b.zig");
}

#[test]
fn reexported_dependency_is_a_module_linking_to_its_shard() {
    let shard = shard("");
    let module = shard
        .symbols
        .iter()
        .find(|s| s.id == "zig:geometry:geometry.units")
        .expect("module");
    assert_eq!(module.kind, SymbolKind::Module as i32);
    assert_eq!(
        module.doc_markdown,
        "Unit helpers.\n\nRe-exports [`units`](dx:zig:units:units)."
    );
}
