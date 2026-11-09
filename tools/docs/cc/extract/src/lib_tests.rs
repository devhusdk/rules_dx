use super::*;

const FILE_XML: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='no'?>
<doxygen version="1.18.0">
  <compounddef id="shapes_8h" kind="file" language="C++">
    <compoundname>shapes.h</compoundname>
    <sectiondef kind="define">
      <memberdef kind="define" id="m_max" prot="public" static="no">
        <name>SHAPES_MAX</name>
        <initializer>64</initializer>
        <briefdescription></briefdescription>
        <detaileddescription><para>Maximum &amp; limit. </para></detaileddescription>
        <location file="include/shapes.h" line="11" column="9"/>
      </memberdef>
    </sectiondef>
    <sectiondef kind="enum">
      <memberdef kind="enum" id="m_kind" prot="public" static="no" strong="no">
        <type></type>
        <name>shape_kind</name>
        <enumvalue id="v_circle" prot="public"><name>SHAPE_CIRCLE</name><briefdescription></briefdescription><detaileddescription><para>A circle. </para></detaileddescription></enumvalue>
        <enumvalue id="v_square" prot="public"><name>SHAPE_SQUARE</name><initializer>= 2</initializer><briefdescription></briefdescription><detaileddescription></detaileddescription></enumvalue>
        <briefdescription></briefdescription>
        <detaileddescription><para>Kind of a shape. </para></detaileddescription>
        <location file="include/shapes.h" line="14" column="1"/>
      </memberdef>
    </sectiondef>
    <sectiondef kind="func">
      <memberdef kind="function" id="m_area" prot="public" static="no" const="no" explicit="no" inline="no" virt="non-virtual">
        <type>double</type>
        <definition>double circle_area</definition>
        <argsstring>(double radius)</argsstring>
        <name>circle_area</name>
        <param><type>double</type><declname>radius</declname></param>
        <briefdescription></briefdescription>
        <detaileddescription><para>Area of a circle. <parameterlist kind="param"><parameteritem><parameternamelist><parametername>radius</parametername></parameternamelist><parameterdescription><para>The radius. </para></parameterdescription></parameteritem></parameterlist><simplesect kind="return"><para>The area. </para></simplesect></para></detaileddescription>
        <location file="include/shapes.h" line="30" column="8" declfile="include/shapes.h" declline="30" declcolumn="8"/>
      </memberdef>
      <memberdef kind="function" id="m_add" prot="public" static="no" const="no" explicit="no" inline="no" virt="non-virtual">
        <type>int</type>
        <definition>int add</definition>
        <argsstring>(int a, int b)</argsstring>
        <name>add</name>
        <param><type>int</type><declname>a</declname></param>
        <param><type>int</type><declname>b</declname></param>
        <briefdescription></briefdescription>
        <detaileddescription></detaileddescription>
        <location file="include/shapes.h" line="33" column="5"/>
      </memberdef>
      <memberdef kind="function" id="m_private" prot="public" static="no" const="no" explicit="no" inline="no" virt="non-virtual">
        <type>int</type>
        <definition>int elsewhere</definition>
        <argsstring>()</argsstring>
        <name>elsewhere</name>
        <briefdescription></briefdescription>
        <detaileddescription></detaileddescription>
        <location file="include/other.h" line="1" column="5"/>
      </memberdef>
    </sectiondef>
  </compounddef>
</doxygen>"#;

const CLASS_XML: &str = r#"<doxygen version="1.18.0">
  <compounddef id="classgeo_1_1Shape" kind="class" language="C++" prot="public" abstract="yes">
    <compoundname>geo::Shape</compoundname>
    <basecompoundref refid="classgeo_1_1Base" prot="public" virt="non-virtual">geo::Base</basecompoundref>
    <sectiondef kind="public-func">
      <memberdef kind="function" id="c_area" prot="public" static="no" const="yes" explicit="no" inline="no" virt="pure-virtual">
        <type>double</type>
        <definition>virtual double geo::Shape::area</definition>
        <argsstring>() const =0</argsstring>
        <name>area</name>
        <briefdescription><para>Area.</para></briefdescription>
        <detaileddescription><para>See <ref refid="x" kindref="member">scale</ref> and <computeroutput>x</computeroutput>.</para></detaileddescription>
        <location file="include/shapes.h" line="45" column="3"/>
      </memberdef>
    </sectiondef>
    <sectiondef kind="private-attrib">
      <memberdef kind="variable" id="c_hidden" prot="private" static="no" mutable="no">
        <type>int</type><name>hidden</name><argsstring></argsstring>
        <briefdescription></briefdescription><detaileddescription></detaileddescription>
        <location file="include/shapes.h" line="50" column="7"/>
      </memberdef>
    </sectiondef>
    <briefdescription><para>A shape.</para></briefdescription>
    <detaileddescription></detaileddescription>
    <location file="include/shapes.h" line="40" column="1" bodyfile="include/shapes.h" bodystart="40" bodyend="51"/>
  </compounddef>
</doxygen>"#;

const BASE_XML: &str = r#"<doxygen version="1.18.0">
  <compounddef id="classgeo_1_1Base" kind="class" language="C++" prot="public">
    <compoundname>geo::Base</compoundname>
    <briefdescription><para>Base.</para></briefdescription>
    <detaileddescription></detaileddescription>
    <location file="include/shapes.h" line="38" column="1"/>
  </compounddef>
</doxygen>"#;

const NAMESPACE_XML: &str = r#"<doxygen version="1.18.0">
  <compounddef id="namespacegeo" kind="namespace" language="C++">
    <compoundname>geo</compoundname>
    <briefdescription><para>Geometry.</para></briefdescription>
    <detaileddescription></detaileddescription>
    <location file="include/shapes.h" line="37" column="1"/>
  </compounddef>
</doxygen>"#;

const MANIFEST: &str = "bazel-out/bin/include/shapes.h\tinclude/shapes.h\t1\ninclude/shapes.h\tinclude/shapes.h\t0\n";

fn shard() -> DocIr {
    convert(
        &[FILE_XML, CLASS_XML, BASE_XML, NAMESPACE_XML],
        MANIFEST,
        "shapes",
        "cc",
    )
    .expect("converts")
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("no {id} in {:?}", shard.symbols.iter().map(|s| &s.id).collect::<Vec<_>>()))
}

#[test]
fn lists_public_symbols_in_id_order() {
    let shard = shard();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "cc:shapes:SHAPES_MAX",
            "cc:shapes:add",
            "cc:shapes:circle_area",
            "cc:shapes:geo",
            "cc:shapes:geo.Base",
            "cc:shapes:geo.Shape",
            "cc:shapes:geo.Shape.area",
            "cc:shapes:shape_kind",
            "cc:shapes:shape_kind.SHAPE_CIRCLE",
            "cc:shapes:shape_kind.SHAPE_SQUARE",
        ]
    );
}

#[test]
fn skips_private_members_and_headers_outside_the_library() {
    let shard = shard();
    assert!(shard.symbols.iter().all(|s| !s.id.ends_with("hidden")));
    assert!(shard.symbols.iter().all(|s| !s.id.ends_with("elsewhere")));
}

#[test]
fn reads_function_params_returns_and_gaps() {
    let shard = shard();
    let area = symbol(&shard, "cc:shapes:circle_area");
    assert_eq!(area.kind, SymbolKind::Function as i32);
    assert_eq!(area.signature_text, "double circle_area(double radius)");
    assert_eq!(area.doc_markdown, "Area of a circle.");
    assert_eq!(area.params[0].name, "radius");
    assert_eq!(area.params[0].r#type, "double");
    assert_eq!(area.params[0].doc, "The radius.");
    assert_eq!(area.returns.as_ref().map(|r| r.doc.as_str()), Some("The area."));
    let add = symbol(&shard, "cc:shapes:add");
    let incomplete = add
        .extensions
        .iter()
        .find(|e| e.key == "cc.incomplete")
        .map(|e| String::from_utf8_lossy(&e.value).into_owned());
    assert_eq!(incomplete.as_deref(), Some("doc;params=a,b;return"));
}

#[test]
fn reads_macros_enums_and_values() {
    let shard = shard();
    let max = symbol(&shard, "cc:shapes:SHAPES_MAX");
    assert_eq!(max.kind, SymbolKind::Macro as i32);
    assert_eq!(max.signature_text, "#define SHAPES_MAX 64");
    assert_eq!(max.doc_markdown, "Maximum & limit.");
    let kind = symbol(&shard, "cc:shapes:shape_kind");
    assert_eq!(kind.kind, SymbolKind::Enum as i32);
    let square = symbol(&shard, "cc:shapes:shape_kind.SHAPE_SQUARE");
    assert_eq!(square.kind, SymbolKind::EnumVariant as i32);
    assert_eq!(square.signature_text, "SHAPE_SQUARE = 2");
    assert_eq!(
        square.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec!["cc:shapes:shape_kind".to_owned()])
    );
}

#[test]
fn nests_classes_and_methods_in_namespaces() {
    let shard = shard();
    let class = symbol(&shard, "cc:shapes:geo.Shape");
    let relations = class.relations.as_ref().expect("relations");
    assert_eq!(relations.member_of, ["cc:shapes:geo"]);
    assert_eq!(relations.implements, ["cc:shapes:geo.Base"]);
    assert_eq!(class.signature_text, "class geo::Shape");
    let method = symbol(&shard, "cc:shapes:geo.Shape.area");
    assert_eq!(method.kind, SymbolKind::Method as i32);
    assert_eq!(method.signature_text, "virtual double area() const =0");
    assert_eq!(method.doc_markdown, "Area.\n\nSee scale and `x`.");
    assert_eq!(method.display_name, "geo::Shape::area");
}

#[test]
fn keeps_source_locations_workspace_relative() {
    let shard = shard();
    let source = symbol(&shard, "cc:shapes:add").source.clone().expect("source");
    assert_eq!(source.file, "include/shapes.h");
    assert_eq!(source.line, 33);
}

#[test]
fn numbers_ids_that_collide() {
    let twice = FILE_XML.replace("<name>add</name>", "<name>circle_area</name>");
    let shard = convert(&[&twice], MANIFEST, "shapes", "cc").expect("converts");
    assert!(shard.symbols.iter().any(|s| s.id == "cc:shapes:circle_area@2"));
}

#[test]
fn rejects_empty_and_bad_input() {
    assert!(convert(&[NAMESPACE_XML], MANIFEST, "shapes", "cc").is_err());
    assert!(convert(&[FILE_XML], MANIFEST, "a:b", "cc").is_err());
    assert!(convert(&[FILE_XML], MANIFEST, "shapes", "d").is_err());
    assert!(convert(&["<doxygen"], MANIFEST, "shapes", "cc").is_err());
    assert!(convert(&["<other/>"], MANIFEST, "shapes", "cc").is_err());
    assert!(convert(&[FILE_XML], "broken line", "shapes", "cc").is_err());
}

#[test]
fn parses_arguments() {
    let args: Vec<String> = [
        "--xml", "x", "--srcs", "s", "--package", "p", "--language", "cuda", "--out", "o",
    ]
    .iter()
    .map(|a| (*a).to_owned())
    .collect();
    let options = parse(&args).expect("parses");
    assert_eq!(options.language, "cuda");
    assert!(parse(&args[..8]).is_err());
    assert!(parse(&["--nope".to_owned(), "x".to_owned()]).is_err());
}

#[test]
fn splits_qualified_names_outside_templates() {
    assert_eq!(segments("a::b<c::d>::e"), ["a", "b<c::d>", "e"]);
}
