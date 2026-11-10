use super::*;

const GRAPH: &str = r#"{
  "metadata": {"formatVersion": {"major": 0, "minor": 6, "patch": 0}, "generator": "test"},
  "module": {"name": "Shapes", "platform": {}},
  "symbols": [
    {
      "kind": {"identifier": "swift.struct", "displayName": "Structure"},
      "identifier": {"precise": "s:6Shapes6CircleV", "interfaceLanguage": "swift"},
      "pathComponents": ["Circle"],
      "docComment": {"lines": [{"text": "A circle."}, {"text": ""}, {"text": "Second line."}]},
      "accessLevel": "public",
      "declarationFragments": [
        {"kind": "keyword", "spelling": "struct"},
        {"kind": "text", "spelling": " "},
        {"kind": "identifier", "spelling": "Circle"}
      ]
    },
    {
      "kind": {"identifier": "swift.property", "displayName": "Instance Property"},
      "identifier": {"precise": "s:6Shapes6CircleV6radiusSdvp", "interfaceLanguage": "swift"},
      "pathComponents": ["Circle", "radius"],
      "docComment": {"lines": [{"text": "The radius."}]},
      "accessLevel": "public",
      "declarationFragments": [
        {"kind": "keyword", "spelling": "var"},
        {"kind": "text", "spelling": " "},
        {"kind": "identifier", "spelling": "radius"},
        {"kind": "text", "spelling": ": "},
        {"kind": "typeIdentifier", "spelling": "Double"}
      ]
    },
    {
      "kind": {"identifier": "swift.method", "displayName": "Instance Method"},
      "identifier": {"precise": "s:6Shapes6CircleV5scale2byACSd_tF", "interfaceLanguage": "swift"},
      "pathComponents": ["Circle", "scale(by:)"],
      "docComment": {"lines": [
        {"text": "Scales the circle."},
        {"text": "- Parameter factor: How much"},
        {"text": "  to grow."},
        {"text": "- Returns: The grown circle."}
      ]},
      "accessLevel": "public",
      "declarationFragments": [
        {"kind": "keyword", "spelling": "func"},
        {"kind": "text", "spelling": " "},
        {"kind": "identifier", "spelling": "scale"},
        {"kind": "text", "spelling": "(by: Double) -> Circle"}
      ],
      "functionSignature": {
        "parameters": [{"name": "by", "internalName": "factor", "declarationFragments": [
          {"kind": "identifier", "spelling": "factor"},
          {"kind": "text", "spelling": ": "},
          {"kind": "typeIdentifier", "spelling": "Double"}
        ]}],
        "returns": [{"kind": "typeIdentifier", "spelling": "Circle"}]
      }
    },
    {
      "kind": {"identifier": "swift.func", "displayName": "Function"},
      "identifier": {"precise": "s:6Shapes4areaySdAA6CircleVF", "interfaceLanguage": "swift"},
      "pathComponents": ["area(_:)"],
      "docComment": {"lines": [{"text": "Area of a circle."}]},
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "func area(_ c: Circle) -> Double"}]
    },
    {
      "kind": {"identifier": "swift.func", "displayName": "Function"},
      "identifier": {"precise": "s:6Shapes4areaySdSdF", "interfaceLanguage": "swift"},
      "pathComponents": ["area(_:)"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "func area(_ r: Double) -> Double"}]
    },
    {
      "kind": {"identifier": "swift.enum", "displayName": "Enumeration"},
      "identifier": {"precise": "s:6Shapes4UnitO", "interfaceLanguage": "swift"},
      "pathComponents": ["Unit"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "enum Unit"}]
    },
    {
      "kind": {"identifier": "swift.enum.case", "displayName": "Case"},
      "identifier": {"precise": "s:6Shapes4UnitO2cmyA2CmF", "interfaceLanguage": "swift"},
      "pathComponents": ["Unit", "cm"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "case cm"}]
    },
    {
      "kind": {"identifier": "swift.protocol", "displayName": "Protocol"},
      "identifier": {"precise": "s:6Shapes8MeasuredP", "interfaceLanguage": "swift"},
      "pathComponents": ["Measured"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "protocol Measured"}]
    },
    {
      "kind": {"identifier": "swift.typealias", "displayName": "Type Alias"},
      "identifier": {"precise": "s:6Shapes6Radiusa", "interfaceLanguage": "swift"},
      "pathComponents": ["Radius"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "typealias Radius = Double"}]
    },
    {
      "kind": {"identifier": "swift.func", "displayName": "Function"},
      "identifier": {"precise": "s:6Shapes7hiddenyyF", "interfaceLanguage": "swift"},
      "pathComponents": ["hidden()"],
      "accessLevel": "internal",
      "declarationFragments": [{"kind": "text", "spelling": "func hidden()"}]
    },
    {
      "kind": {"identifier": "swift.extension", "displayName": "Extension"},
      "identifier": {"precise": "s:e:s:6Shapes6CircleV", "interfaceLanguage": "swift"},
      "pathComponents": ["Circle"],
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "extension Circle"}]
    }
  ],
  "relationships": []
}"#;

const FIRST: &str = "/// A circle.\npublic struct Circle {\n    public var radius: Double\n    public func scale(by factor: Double) -> Circle { self }\n}\n\npublic typealias Radius = Double\n";
const SECOND: &str = "public func area(_ c: Circle) -> Double { 0 }\npublic func area(_ r: Double) -> Double { r }\n// enum Unit is not here\npublic enum Unit {\n    case mm, cm\n}\npublic protocol Measured {}\n";

fn sources() -> (Vec<Source>, BTreeMap<String, String>) {
    let list = vec![
        Source {
            exec_path: "shapes/First.swift".to_owned(),
            rel: "shapes/First.swift".to_owned(),
            generated: false,
        },
        Source {
            exec_path: "bazel-out/Second.swift".to_owned(),
            rel: "shapes/Second.swift".to_owned(),
            generated: true,
        },
    ];
    let mut texts = BTreeMap::new();
    texts.insert("shapes/First.swift".to_owned(), FIRST.to_owned());
    texts.insert("bazel-out/Second.swift".to_owned(), SECOND.to_owned());
    (list, texts)
}

fn shard(config: &str) -> DocIr {
    let (list, texts) = sources();
    convert(GRAPH, &[], "Shapes", &list, &texts, "shapes", config).expect("converts")
}

fn find<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == format!("swift:shapes:{id}"))
        .unwrap_or_else(|| {
            panic!(
                "no {id} in {:?}",
                shard.symbols.iter().map(|s| &s.id).collect::<Vec<_>>()
            )
        })
}

fn at(shard: &DocIr, id: &str) -> (String, u64) {
    let source = find(shard, id).source.clone().expect("source");
    (source.file, source.line)
}

#[test]
fn ids_kinds_and_skipped_symbols() {
    let shard = shard("");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 10, "{ids:?}");
    assert_eq!(find(&shard, "Shapes").kind, SymbolKind::Module as i32);
    assert_eq!(find(&shard, "Shapes.Circle").kind, SymbolKind::Class as i32);
    assert_eq!(
        find(&shard, "Shapes.Circle.radius").kind,
        SymbolKind::Property as i32
    );
    assert_eq!(
        find(&shard, "Shapes.Circle.scale~28by~3A~29").kind,
        SymbolKind::Method as i32
    );
    assert_eq!(find(&shard, "Shapes.Unit").kind, SymbolKind::Enum as i32);
    assert_eq!(
        find(&shard, "Shapes.Unit.cm").kind,
        SymbolKind::EnumVariant as i32
    );
    assert_eq!(
        find(&shard, "Shapes.Measured").kind,
        SymbolKind::Interface as i32
    );
    assert_eq!(
        find(&shard, "Shapes.Radius").kind,
        SymbolKind::TypeAlias as i32
    );
    assert!(!ids.iter().any(|id| id.contains("hidden")));
    let relations = find(&shard, "Shapes.Circle.radius")
        .relations
        .clone()
        .expect("relations");
    assert_eq!(
        relations.member_of,
        vec!["swift:shapes:Shapes.Circle".to_owned()]
    );
}

#[test]
fn overloads_get_numbered_ids() {
    let shard = shard("");
    let first = find(&shard, "Shapes.area~28_~3A~29");
    let second = find(&shard, "Shapes.area~28_~3A~29@2");
    assert_eq!(first.display_name, "area(_:)");
    assert_eq!(first.signature_text, "func area(_ c: Circle) -> Double");
    assert_eq!(second.signature_text, "func area(_ r: Double) -> Double");
    assert_eq!(
        at(&shard, "Shapes.area~28_~3A~29"),
        ("shapes/Second.swift".to_owned(), 1)
    );
    assert_eq!(
        at(&shard, "Shapes.area~28_~3A~29@2"),
        ("shapes/Second.swift".to_owned(), 2)
    );
}

#[test]
fn docs_params_returns_and_positions() {
    let shard = shard("x");
    assert_eq!(
        find(&shard, "Shapes.Circle").doc_markdown,
        "A circle.\n\nSecond line."
    );
    assert_eq!(
        find(&shard, "Shapes.Circle.radius").signature_text,
        "var radius: Double"
    );
    assert_eq!(find(&shard, "Shapes.Unit").doc_markdown, "");
    let scale = find(&shard, "Shapes.Circle.scale~28by~3A~29");
    assert_eq!(scale.doc_markdown, "Scales the circle.");
    assert_eq!(scale.params.len(), 1);
    assert_eq!(scale.params[0].name, "factor");
    assert_eq!(scale.params[0].r#type, "Double");
    assert_eq!(scale.params[0].doc, "How much to grow.");
    let returns = scale.returns.clone().expect("returns");
    assert_eq!(
        (returns.r#type.as_str(), returns.doc.as_str()),
        ("Circle", "The grown circle.")
    );
    assert_eq!(
        at(&shard, "Shapes.Circle"),
        ("shapes/First.swift".to_owned(), 2)
    );
    assert_eq!(
        at(&shard, "Shapes.Circle.radius"),
        ("shapes/First.swift".to_owned(), 3)
    );
    assert_eq!(at(&shard, "Shapes.Circle.scale~28by~3A~29").1, 4);
    assert_eq!(
        at(&shard, "Shapes.Radius"),
        ("shapes/First.swift".to_owned(), 7)
    );
    assert_eq!(
        at(&shard, "Shapes.Unit"),
        ("shapes/Second.swift".to_owned(), 4)
    );
    assert_eq!(
        at(&shard, "Shapes.Unit.cm"),
        ("shapes/Second.swift".to_owned(), 5)
    );
    let generated = find(&shard, "Shapes.Unit")
        .source
        .clone()
        .expect("source")
        .generated;
    assert!(generated);
    assert_eq!(
        find(&shard, "Shapes").extensions[0].value,
        b"symbolgraph=1;package=shapes;module=Shapes;config=x"
    );
}

#[test]
fn undocumented_symbol_has_empty_doc() {
    let shard = shard("");
    let symbol = find(&shard, "Shapes.Measured");
    assert!(symbol.doc_markdown.is_empty());
    assert!(symbol.params.is_empty());
    assert!(symbol.returns.is_none());
}

#[test]
fn wrong_module_and_bad_json_are_rejected() {
    let (list, texts) = sources();
    let error = convert(GRAPH, &[], "Other", &list, &texts, "shapes", "").expect_err("rejected");
    assert!(error.contains("not 'Other'"), "{error}");
    let error = convert("{", &[], "Shapes", &list, &texts, "shapes", "").expect_err("rejected");
    assert!(error.contains("bad symbol graph"), "{error}");
}

#[test]
fn parameters_block_is_split() {
    let lines: Vec<String> = [
        "Does it.",
        "- Parameters:",
        "  - a: First.",
        "  - b: Second.",
        "",
        "More.",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let (doc, params, returns) = split_doc(&lines);
    assert_eq!(doc, "Does it.\n\nMore.");
    assert_eq!(
        params,
        vec![
            ("a".to_owned(), "First.".to_owned()),
            ("b".to_owned(), "Second.".to_owned())
        ]
    );
    assert!(returns.is_none());
}

#[test]
fn parse_rejects_bad_arguments() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(parse(&args(&["--nope", "x"])).is_err());
    assert!(parse(&args(&["--graphs", "a"])).is_err());
    assert!(parse(&args(&["--graphs", "a", "--graphs", "b"])).is_err());
    let all = args(&[
        "--graphs",
        "a",
        "--sources",
        "s",
        "--module",
        "m",
        "--out",
        "o",
        "--package",
        "p",
    ]);
    assert_eq!(parse(&all).expect("parses").package, "p");
}

const EXTENSION_GRAPH: &str = r#"{
  "metadata": {"formatVersion": {"major": 0, "minor": 6, "patch": 0}, "generator": "test"},
  "module": {"name": "Shapes", "platform": {}},
  "symbols": [
    {
      "kind": {"identifier": "swift.method", "displayName": "Instance Method"},
      "identifier": {"precise": "s:SS6ShapesE5shoutSSyF", "interfaceLanguage": "swift"},
      "pathComponents": ["String", "shout()"],
      "docComment": {"lines": [{"text": "Upper-cases the string."}]},
      "accessLevel": "public",
      "declarationFragments": [{"kind": "text", "spelling": "func shout() -> String"}],
      "functionSignature": {"returns": [{"kind": "typeIdentifier", "spelling": "String"}]}
    },
    {
      "kind": {"identifier": "swift.method", "displayName": "Instance Method"},
      "identifier": {"precise": "s:SS6ShapesE6quietyyF", "interfaceLanguage": "swift"},
      "pathComponents": ["String", "quiet()"],
      "accessLevel": "internal",
      "declarationFragments": [{"kind": "text", "spelling": "func quiet()"}]
    }
  ],
  "relationships": [
    {"kind": "memberOf", "source": "s:SS6ShapesE5shoutSSyF", "target": "s:SS"},
    {"kind": "memberOf", "source": "s:SS6ShapesE6quietyyF", "target": "s:SS"}
  ]
}"#;

#[test]
fn members_added_to_other_modules_types_are_documented() {
    let (list, mut texts) = sources();
    texts.insert(
        "shapes/First.swift".to_owned(),
        format!("{FIRST}public extension String {{\n    func shout() -> String {{ self }}\n}}\n"),
    );
    let extensions = vec![("Swift".to_owned(), EXTENSION_GRAPH.to_owned())];
    let shard =
        convert(GRAPH, &extensions, "Shapes", &list, &texts, "shapes", "").expect("converts");
    let shout = find(&shard, "Swift.String.shout~28~29");
    assert_eq!(shout.kind, SymbolKind::Method as i32);
    assert_eq!(shout.doc_markdown, "Upper-cases the string.");
    assert_eq!(shout.returns.clone().expect("returns").r#type, "String");
    assert_eq!(
        shout.relations.clone().expect("relations").member_of,
        vec!["swift:shapes:Shapes".to_owned()]
    );
    let extends = shout
        .extensions
        .iter()
        .find(|extension| extension.key == "swift.extends")
        .expect("extends");
    assert_eq!(extends.value, b"s:SS");
    assert_eq!(at(&shard, "Swift.String.shout~28~29").1, 9);
    assert!(!shard.symbols.iter().any(|s| s.id.contains("quiet")));
}
