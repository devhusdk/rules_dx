//! Checks the C# fixture shards: shapes, signatures, docs, sources and configuration.

use std::path::PathBuf;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "csharp:Dx.Fixture.Ledger:Dx.Fixture.Ledger.";
const ACCOUNT: &str = "tools/docs/dotnet/fixture/csharp/ledger/Account.cs";

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

#[test]
fn the_fixture_shard_maps_every_shape() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "csharp");
    assert_eq!(shard.package, "Dx.Fixture.Ledger");
    for (name, kind) in [
        ("Account", SymbolKind::Class),
        ("Account.Balance", SymbolKind::Property),
        ("Account.Changed", SymbolKind::Property),
        ("Account.GetCurrency", SymbolKind::Method),
        ("Account.Larger~3CT~3E", SymbolKind::Method),
        ("AccountKind", SymbolKind::Enum),
        ("AccountKind.Savings", SymbolKind::EnumVariant),
        ("Entry", SymbolKind::Class),
        ("Entry.Cents", SymbolKind::Field),
        ("IAuditable", SymbolKind::Interface),
        ("Names", SymbolKind::Class),
    ] {
        assert_eq!(symbol(&shard, name).kind(), kind, "{name}");
    }
    assert_eq!(symbols(&shard, "Account.Deposit").len(), 2);
    symbol(&shard, "Plain.X");
    symbol(&shard, "Point");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert!(
        !ids.iter().any(|id| id.contains("Hidden")),
        "an internal type leaked: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.contains("Freeze")),
        "a conditional member leaked: {ids:?}"
    );
}

#[test]
fn signatures_docs_and_sources_come_from_the_compile() {
    let shard = shard("DX_FIXTURE_SHARD");
    let deposit = overload(
        &shard,
        "Account.Deposit",
        "public decimal Deposit(long cents)",
    );
    assert_eq!(
        deposit.doc_markdown.lines().next(),
        Some("Adds a whole number of cents to the balance.")
    );
    let source = deposit.source.as_ref().expect("a source");
    assert_eq!((source.file.as_str(), source.line), (ACCOUNT, 47));
    assert!(!source.generated);
    let larger = symbol(&shard, "Account.Larger~3CT~3E");
    assert_eq!(
        larger.signature_text,
        "public static T Larger<T>(T left, T right) where T : IComparable<T>"
    );
    let names = symbol(&shard, "Names.Find");
    assert_eq!(
        names.signature_text,
        "public static string? Find(string key)"
    );
    let audit = symbol(&shard, "Account.Audit");
    assert!(audit.doc_markdown.contains("Audits the instance."));
    let account = symbol(&shard, "Account");
    assert_eq!(account.signature_text, "public class Account : IAuditable");
    assert!(account.doc_markdown.contains("not thread safe"));
    assert_eq!(
        account.relations.as_ref().expect("relations").implements,
        vec![format!("{PREFIX}IAuditable")]
    );
}

#[test]
fn the_defines_select_the_members() {
    let base = shard("DX_FIXTURE_SHARD");
    let extra = shard("DX_FIXTURE_EXTRA_SHARD");
    assert!(symbols(&base, "Account.Freeze").is_empty());
    assert_eq!(symbols(&extra, "Account.Freeze").len(), 1);
    assert_ne!(
        std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("read"),
        std::fs::read(runfile("DX_FIXTURE_EXTRA_SHARD")).expect("read")
    );
}
