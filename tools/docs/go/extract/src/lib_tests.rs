use super::*;
use serde_json::json;

fn item(kind: &str, name: &str, recv: &str, type_kind: &str) -> Value {
    json!({
        "kind": kind, "name": name, "recv": recv, "type_kind": type_kind,
        "signature": format!("sig {name}"), "doc": format!("Doc of {name}."),
        "file": "pkg/a.go", "line": 7, "generated": false
    })
}

fn document(items: Value) -> Value {
    json!({
        "schema": 1, "importpath": "example.com/pkg", "package": "pkg",
        "config": "goos=linux;goarch=amd64;tags=;cgo=0;json=1",
        "excluded": ["pkg/c.go", "pkg/w.go"], "doc": "Package pkg.",
        "file": "pkg/a.go", "line": 1, "generated": false, "items": items
    })
}

fn full() -> Value {
    document(json!([
        item("type", "Account", "", "struct"),
        item("method", "Deposit", "Account", ""),
        item("field", "Owner", "Account", ""),
        item("type", "Store", "", "interface"),
        item("method", "Get", "Store", ""),
        item("type", "Name", "", "alias"),
        item("type", "Kind", "", "other"),
        item("func", "Open", "", ""),
        item("const", "Limit", "", ""),
        item("var", "Default", "", "")
    ]))
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn every_kind_maps_onto_the_ir() {
    let shard = convert(&full().to_string()).unwrap();
    assert_eq!(shard.language, "go");
    assert_eq!(shard.package, "example.com/pkg");
    for (id, kind) in [
        ("go:example.com/pkg:pkg", SymbolKind::Module),
        ("go:example.com/pkg:Account", SymbolKind::Class),
        ("go:example.com/pkg:Account.Deposit", SymbolKind::Method),
        ("go:example.com/pkg:Account.Owner", SymbolKind::Field),
        ("go:example.com/pkg:Store", SymbolKind::Interface),
        ("go:example.com/pkg:Store.Get", SymbolKind::Method),
        ("go:example.com/pkg:Name", SymbolKind::TypeAlias),
        ("go:example.com/pkg:Kind", SymbolKind::Class),
        ("go:example.com/pkg:Open", SymbolKind::Function),
        ("go:example.com/pkg:Limit", SymbolKind::Constant),
        ("go:example.com/pkg:Default", SymbolKind::Variable),
    ] {
        let found = symbol(&shard, id);
        assert_eq!(found.kind(), kind, "{id}");
        assert_eq!(found.visibility(), Visibility::Public, "{id}");
    }
    assert_eq!(shard.symbols.len(), 11);
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn members_link_to_their_type_and_carry_source_and_config() {
    let shard = convert(&full().to_string()).unwrap();
    let deposit = symbol(&shard, "go:example.com/pkg:Account.Deposit");
    assert_eq!(
        deposit.relations.as_ref().unwrap().member_of,
        vec!["go:example.com/pkg:Account".to_owned()]
    );
    assert_eq!(deposit.signature_text, "sig Deposit");
    assert_eq!(deposit.doc_markdown, "Doc of Deposit.");
    let source = deposit.source.as_ref().unwrap();
    assert_eq!((source.file.as_str(), source.line), ("pkg/a.go", 7));
    assert_eq!(deposit.extensions.len(), 1);
    assert_eq!(deposit.extensions[0].key, "go.config");
    let module = symbol(&shard, "go:example.com/pkg:pkg");
    let keys: Vec<&str> = module.extensions.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["go.config", "go.excluded"]);
    assert_eq!(module.extensions[1].value, b"pkg/c.go\npkg/w.go");
    assert_eq!(
        module.signature_text,
        "package pkg // import \"example.com/pkg\""
    );
}

#[test]
fn malformed_documents_are_rejected() {
    let cases: Vec<(Value, &str)> = vec![
        (json!([]), "not an object"),
        (
            {
                let mut doc = full();
                doc["schema"] = json!(2);
                doc
            },
            "schema 2 is not supported",
        ),
        (
            {
                let mut doc = full();
                doc.as_object_mut().unwrap().remove("config");
                doc
            },
            "has no 'config'",
        ),
        (
            {
                let mut doc = full();
                doc["importpath"] = json!("a:b");
                doc
            },
            "invalid importpath",
        ),
        (document(json!([])), "no exported symbols"),
        (
            document(json!([item("thing", "X", "", "")])),
            "unknown kind 'thing'",
        ),
        (
            document(json!([item("type", "X", "", "union")])),
            "unknown type_kind",
        ),
        (document(json!([item("method", "X", "", "")])), "receiver"),
        (document(json!([item("func", "X", "T", "")])), "receiver"),
        (
            document(json!([item("func", "X", "", ""), item("var", "X", "", "")])),
            "DuplicateSymbolId",
        ),
        (
            {
                let mut doc = document(json!([item("func", "X", "", "")]));
                doc["items"][0]["file"] = json!("/abs/a.go");
                doc
            },
            "UnsafeSourcePath",
        ),
    ];
    for (doc, want) in cases {
        let error = convert(&doc.to_string()).unwrap_err();
        assert!(error.contains(want), "{error} does not mention {want}");
    }
    assert!(convert("{").unwrap_err().contains("malformed"));
}

#[test]
fn argv_is_parsed_strictly() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        parse(&args(&["--json", "a.json", "--out", "a.pb"])),
        Ok(Options {
            json: "a.json".to_owned(),
            out: "a.pb".to_owned(),
        })
    );
    assert!(parse(&args(&["--json", "a.json"])).is_err());
    assert!(parse(&args(&["--json", "a", "--json", "b", "--out", "c"])).is_err());
    assert!(parse(&args(&["--bogus", "a"])).is_err());
    assert!(parse(&args(&["--out"])).is_err());
    assert!(parse(&args(&["--json", "", "--out", "c"])).is_err());
}

#[test]
fn run_writes_nothing_on_failure() {
    let base = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, Into::into);
    let dir = base.join(format!("go-extract-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let json = dir.join("in.json");
    let out = dir.join("out.pb");
    fs::write(&json, document(json!([])).to_string()).unwrap();
    let options = Options {
        json: json.display().to_string(),
        out: out.display().to_string(),
    };
    assert!(run(&options).is_err());
    assert!(!out.exists());
    fs::write(&json, full().to_string()).unwrap();
    run(&options).unwrap();
    let decoded = documentation_ir::decode_shard(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(decoded, convert(&full().to_string()).unwrap());
}
