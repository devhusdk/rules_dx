//! Checks the F# fixture shards: shapes, signatures, docs, sources and configuration.

use std::path::PathBuf;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "fsharp:Dx.Fixture.FsLedger:Dx.Fixture.FsLedger.";
const LEDGER: &str = "tools/docs/dotnet/fixture/fsharp/ledger/Ledger.fs";

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard(var: &str) -> DocIr {
    let bytes = std::fs::read(runfile(var)).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

fn symbols<'a>(shard: &'a DocIr, name: &str) -> Vec<&'a Symbol> {
    let id = format!("{PREFIX}{name}");
    let overload = format!("{id}~~");
    shard
        .symbols
        .iter()
        .filter(|s| s.id == id || s.id.starts_with(&overload))
        .collect()
}

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    let found = symbols(shard, name);
    assert_eq!(found.len(), 1, "{name} must name exactly one symbol");
    found[0]
}

fn overload<'a>(shard: &'a DocIr, name: &str, signature: &str) -> &'a Symbol {
    symbols(shard, name)
        .into_iter()
        .find(|s| s.signature_text == signature)
        .unwrap_or_else(|| panic!("{name} has no overload {signature}"))
}

fn extension(symbol: &Symbol, key: &str) -> String {
    let found = symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .unwrap_or_else(|| panic!("{} has no {key}", symbol.id));
    String::from_utf8(found.value.clone()).expect("utf-8 extension")
}

#[test]
fn the_fixture_shard_maps_every_shape() {
    let shard = shard("DX_FIXTURE_FSHARP_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "fsharp");
    assert_eq!(shard.package, "Dx.Fixture.FsLedger");
    for (name, kind) in [
        ("Account", SymbolKind::Class),
        ("Account.Balance", SymbolKind::Property),
        ("Account.Currency", SymbolKind::Property),
        ("Account.new", SymbolKind::Method),
        ("AccountKind", SymbolKind::Enum),
        ("AccountKind.Savings", SymbolKind::EnumVariant),
        ("AccountModule", SymbolKind::Module),
        ("AccountModule.create", SymbolKind::Function),
        ("Amounts", SymbolKind::Module),
        ("Amounts.add", SymbolKind::Function),
        ("Cents", SymbolKind::TypeAlias),
        ("Entry", SymbolKind::Class),
        ("Entry.Memo", SymbolKind::Field),
        ("IAuditable", SymbolKind::Interface),
        ("IAuditable.Audit", SymbolKind::Method),
        ("Outcome", SymbolKind::Enum),
        ("Outcome.Refused", SymbolKind::EnumVariant),
    ] {
        assert_eq!(symbol(&shard, name).kind(), kind, "{name}");
    }
    assert_eq!(symbols(&shard, "Account.Deposit").len(), 2);
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    for hidden in ["Hidden", "hidden", "visibleInside", "extra"] {
        assert!(
            !ids.iter().any(|id| id.contains(hidden)),
            "{hidden} leaked: {ids:?}"
        );
    }
}

#[test]
fn signatures_docs_and_sources_come_from_the_compile() {
    let shard = shard("DX_FIXTURE_FSHARP_SHARD");
    let deposit = overload(
        &shard,
        "Account.Deposit",
        "member this.Deposit(cents: int64) : decimal",
    );
    assert_eq!(
        deposit.doc_markdown.lines().next(),
        Some("Adds a whole number of cents to the balance.")
    );
    let source = deposit.source.as_ref().expect("a source");
    assert_eq!((source.file.as_str(), source.line), (LEDGER, 54));
    assert!(!source.generated);
    assert_eq!(
        symbol(&shard, "Amounts.add").signature_text,
        "let add (left: int64) (right: int64) : int64"
    );
    assert_eq!(
        symbol(&shard, "Outcome.Refused").signature_text,
        "| Refused of reason: string * code: int"
    );
    assert_eq!(
        symbol(&shard, "Account.Balance").signature_text,
        "member this.Balance : decimal with get, set"
    );
    let account = symbol(&shard, "Account");
    assert!(account.doc_markdown.contains("not thread safe"));
}

#[test]
fn undocumented_members_are_marked_incomplete() {
    let shard = shard("DX_FIXTURE_FSHARP_SHARD");
    let undocumented = symbol(&shard, "Amounts.undocumented");
    assert!(extension(undocumented, "fsharp.incomplete").contains("summary"));
    assert!(extension(undocumented, "fsharp.config").starts_with("assembly="));
}

#[test]
fn the_defines_select_the_members() {
    let base = shard("DX_FIXTURE_FSHARP_SHARD");
    let extra = shard("DX_FIXTURE_FSHARP_EXTRA_SHARD");
    assert!(symbols(&base, "Amounts.extra").is_empty());
    let found = symbol(&extra, "Amounts.extra");
    assert!(extension(found, "fsharp.config").contains("defines=DX_EXTRA"));
    assert_ne!(
        std::fs::read(runfile("DX_FIXTURE_FSHARP_SHARD")).expect("read"),
        std::fs::read(runfile("DX_FIXTURE_FSHARP_EXTRA_SHARD")).expect("read")
    );
}
