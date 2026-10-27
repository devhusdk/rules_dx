//! Checks the Go fixture shards and the driver's and converter's failure handling.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

const PREFIX: &str = "go:example.com/ledger/money:";
const SOURCE: &str = "tools/docs/go/fixture/money.go";

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard(var: &str) -> DocIr {
    let bytes = std::fs::read(runfile(var)).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    let id = format!("{PREFIX}{name}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

fn extension<'a>(symbol: &'a Symbol, key: &str) -> &'a str {
    let found = symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .unwrap_or_else(|| panic!("{} has no {key}", symbol.id));
    std::str::from_utf8(&found.value).expect("utf-8 extension")
}

#[test]
fn the_fixture_shard_maps_every_shape() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "go");
    assert_eq!(shard.package, "example.com/ledger/money");
    for (name, kind) in [
        ("money", SymbolKind::Module),
        ("Amount", SymbolKind::Class),
        ("Account", SymbolKind::Class),
        ("Account.Deposit", SymbolKind::Method),
        ("Account.Owner", SymbolKind::Field),
        ("Cents", SymbolKind::TypeAlias),
        ("Currency", SymbolKind::Class),
        ("DefaultCurrency", SymbolKind::Variable),
        ("ErrOverdrawn", SymbolKind::Variable),
        ("EUR", SymbolKind::Constant),
        ("USD", SymbolKind::Constant),
        ("MaxAmount", SymbolKind::Constant),
        ("Ledger", SymbolKind::Interface),
        ("Ledger.Find", SymbolKind::Method),
        ("NewAccount", SymbolKind::Function),
        ("Pair", SymbolKind::Class),
        ("Pair.Key", SymbolKind::Field),
        ("Sum", SymbolKind::Function),
    ] {
        assert_eq!(symbol(&shard, name).kind(), kind, "{name}");
    }
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    for hidden in ["audit", "balance", "round", "WindowsOnly"] {
        assert!(
            !ids.iter().any(|id| id.ends_with(hidden)),
            "{hidden} leaked: {ids:?}"
        );
    }
}

#[test]
fn signatures_docs_and_sources_come_from_go_doc() {
    let shard = shard("DX_FIXTURE_SHARD");
    let deposit = symbol(&shard, "Account.Deposit");
    assert_eq!(
        deposit.signature_text,
        "func (a *Account) Deposit(amount Amount)"
    );
    assert_eq!(
        deposit.doc_markdown,
        "Deposit adds an amount to the balance."
    );
    let source = deposit.source.as_ref().expect("a source");
    assert_eq!((source.file.as_str(), source.line), (SOURCE, 47));
    assert!(!source.generated);
    assert_eq!(
        deposit.relations.as_ref().expect("relations").member_of,
        vec![format!("{PREFIX}Account")]
    );
    let sum = symbol(&shard, "Sum");
    assert_eq!(sum.signature_text, "func Sum[T ~int64](values ...T) T");
    let account = symbol(&shard, "Account");
    assert!(account.signature_text.contains("unexported fields"));
    assert!(!account.signature_text.contains("balance"));
    let usd = symbol(&shard, "USD");
    assert!(usd.signature_text.contains("EUR Currency = iota"));
    assert_eq!(usd.doc_markdown, "The supported currencies.");
    let module = symbol(&shard, "money");
    assert!(module.doc_markdown.contains("# Accounts"));
    assert!(!module.doc_markdown.contains("pkg.go.dev"));
    for symbol in &shard.symbols {
        let source = symbol.source.as_ref().expect("every symbol has a source");
        assert_eq!(source.file, SOURCE, "{}", symbol.id);
        assert!(source.line > 0);
        assert_eq!(
            extension(symbol, "go.config"),
            "goos=linux;goarch=amd64;tags=;cgo=0;json=1"
        );
    }
    assert_eq!(
        extension(module, "go.excluded"),
        "tools/docs/go/fixture/windows.go"
    );
}

#[test]
fn build_constraints_follow_the_configured_goos() {
    let windows = shard("DX_FIXTURE_WINDOWS_SHARD");
    let only = symbol(&windows, "WindowsOnly");
    assert_eq!(
        only.source.as_ref().map(|s| s.file.as_str()),
        Some("tools/docs/go/fixture/windows.go")
    );
    let module = symbol(&windows, "money");
    assert_eq!(extension(module, "go.excluded"), "");
    assert_eq!(
        extension(module, "go.config"),
        "goos=windows;goarch=amd64;tags=;cgo=0;json=1"
    );
}

#[test]
fn a_changed_doc_changes_the_shard_bytes() {
    let base = std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("read");
    let changed = std::fs::read(runfile("DX_FIXTURE_CHANGED_SHARD")).expect("read");
    assert_ne!(base, changed);
    let changed = decode_shard(&changed).expect("decodes");
    assert_eq!(
        symbol(&changed, "Account.Deposit").doc_markdown,
        "Deposit adds an amount to the balance and never fails."
    );
}

fn run(var: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(runfile(var))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run the tool");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_driver_fails_on_a_broken_package_and_writes_nothing() {
    let dir = dx_testing::mkscratch("go-driver").expect("scratch");
    std::fs::write(dir.join("broken.go"), "package broken\n\nfunc Broken( {\n").expect("write");
    std::fs::write(dir.join("srcs.tsv"), "broken.go\tpkg/broken.go\t0\n").expect("write");
    let (code, stderr) = run(
        "DX_GODOC_JSON",
        &dir,
        &[
            "--importpath",
            "example.com/broken",
            "--srcs-file",
            "srcs.tsv",
            "--out",
            "out.json",
        ],
    );
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("cannot parse pkg/broken.go"), "{stderr}");
    assert!(!dir.join("out.json").exists());
}

#[test]
fn the_converter_rejects_bad_json_and_writes_no_shard() {
    let dir = dx_testing::mkscratch("go-extract").expect("scratch");
    let json = std::fs::read_to_string(runfile("DX_FIXTURE_JSON")).expect("driver json");
    assert!(json.contains("\"schema\": 1,"));
    std::fs::write(
        dir.join("new.json"),
        json.replace("\"schema\": 1,", "\"schema\": 9,"),
    )
    .expect("write");
    let (code, stderr) = run(
        "DX_GO_EXTRACT",
        &dir,
        &["--json", "new.json", "--out", "shard.ir.pb"],
    );
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("schema 9 is not supported"), "{stderr}");
    assert!(!dir.join("shard.ir.pb").exists());
    std::fs::write(dir.join("ok.json"), &json).expect("write");
    let (code, stderr) = run(
        "DX_GO_EXTRACT",
        &dir,
        &["--json", "ok.json", "--out", "shard.ir.pb"],
    );
    assert_eq!(code, 0, "{stderr}");
    let written = std::fs::read(dir.join("shard.ir.pb")).expect("shard");
    assert_eq!(
        written,
        std::fs::read(runfile("DX_FIXTURE_SHARD")).expect("fixture")
    );
}
