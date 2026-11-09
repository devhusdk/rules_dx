use super::*;
use serde_json::json;

fn item(name: &str, kind: u64, href: &str, desc: &str, parent: Option<(&str, u64, &str)>) -> Value {
    let mut value = json!({"name": name, "qualifiedName": name, "href": href, "kind": kind, "desc": desc});
    if let Some((pname, pkind, phref)) = parent {
        value["enclosedBy"] = json!({"name": pname, "kind": pkind, "href": phref});
    }
    value
}

fn index() -> String {
    json!([
        item("ledger", 9, "ledger/", "A ledger library.", None),
        item("Ledger", 3, "ledger/Ledger-class.html", "Adds entries.", Some(("ledger", 9, "ledger/"))),
        item("Ledger.new", 2, "ledger/Ledger/Ledger.html", "Creates one.", Some(("Ledger", 3, "ledger/Ledger-class.html"))),
        item("add", 10, "ledger/Ledger/add.html", "Adds n.", Some(("Ledger", 3, "ledger/Ledger-class.html"))),
        item("Color", 5, "ledger/Color.html", "", Some(("ledger", 9, "ledger/"))),
        item("red", 1, "ledger/Color/red-constant.html", "Red.", Some(("Color", 5, "ledger/Color.html"))),
        item("M", 11, "ledger/M-mixin.html", "A mixin.", Some(("ledger", 9, "ledger/"))),
        item("toString", 10, "ledger/M/toString.html", "A string representation of this object.", Some(("M", 11, "ledger/M-mixin.html"))),
        item("answer", 19, "ledger/answer-constant.html", "A const.", Some(("ledger", 9, "ledger/")))
    ])
    .to_string()
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn converts_every_declaration_kind() {
    let shard = convert(&index(), "ledger").unwrap();
    assert_eq!(
        ids(&shard),
        [
            "dart:ledger:ledger",
            "dart:ledger:ledger.Color",
            "dart:ledger:ledger.Color.red",
            "dart:ledger:ledger.Ledger",
            "dart:ledger:ledger.Ledger.Ledger~2Enew",
            "dart:ledger:ledger.Ledger.add",
            "dart:ledger:ledger.M",
            "dart:ledger:ledger.answer",
        ]
    );
    let kinds: Vec<(&str, i32)> = shard.symbols.iter().map(|s| (s.display_name.as_str(), s.kind)).collect();
    assert!(kinds.contains(&("Ledger", SymbolKind::Class as i32)));
    assert!(kinds.contains(&("Ledger.new", SymbolKind::Method as i32)));
    assert!(kinds.contains(&("red", SymbolKind::EnumVariant as i32)));
    assert!(kinds.contains(&("M", SymbolKind::Interface as i32)));
}

#[test]
fn nests_members_under_their_owner() {
    let shard = convert(&index(), "ledger").unwrap();
    let add = shard.symbols.iter().find(|s| s.display_name == "add").unwrap();
    assert_eq!(add.id, "dart:ledger:ledger.Ledger.add");
    assert_eq!(add.relations.as_ref().unwrap().member_of, ["dart:ledger:ledger.Ledger"]);
    assert_eq!(add.doc_markdown, "Adds n.");
}

#[test]
fn drops_object_members_of_mixins() {
    let shard = convert(&index(), "ledger").unwrap();
    assert!(!ids(&shard).iter().any(|id| id.ends_with("toString")));
}

#[test]
fn marks_undocumented_symbols_incomplete() {
    let shard = convert(&index(), "ledger").unwrap();
    let color = shard.symbols.iter().find(|s| s.display_name == "Color").unwrap();
    assert!(color.extensions.iter().any(|e| e.key == KEY_INCOMPLETE));
}

#[test]
fn rejects_bad_input() {
    assert!(convert("{}", "ledger").is_err());
    assert!(convert("[]", "ledger").is_err());
    assert!(convert(&index(), "bad name").is_err());
    let orphan = json!([item("a", 3, "x.html", "", Some(("b", 9, "missing/")))]).to_string();
    assert!(convert(&orphan, "ledger").unwrap_err().contains("unknown"));
}

#[test]
fn parses_arguments() {
    let args: Vec<String> = ["--index", "i", "--package", "p", "--out", "o"].map(String::from).to_vec();
    assert_eq!(parse(&args).unwrap().package, "p");
    assert!(parse(&args[..4]).is_err());
    assert!(parse(&["--nope".to_owned()]).is_err());
}
