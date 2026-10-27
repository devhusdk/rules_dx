//! Checks the PowerShell fixture shards and the mapper's failure handling.

use std::path::PathBuf;
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol};
use documentation_ir::{decode_shard, encode_shard, validate_shard};

const LEDGER_DIR: &str = "tools/docs/powershell/fixture/Ledger";

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn read_shard(var: &str) -> (Vec<u8>, DocIr) {
    let bytes = std::fs::read(runfile(var)).expect("read the shard");
    let shard = decode_shard(&bytes).expect("the shard decodes");
    (bytes, shard)
}

fn ledger() -> DocIr {
    read_shard("DX_LEDGER_SHARD").1
}

fn symbol<'a>(shard: &'a DocIr, name: &str) -> &'a Symbol {
    let id = format!("powershell:{}:{name}", shard.package);
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("no symbol {id}"))
}

fn extension(symbol: &Symbol, key: &str) -> Option<String> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| String::from_utf8(e.value.clone()).expect("utf8 extension"))
}

fn assert_config(value: &str, manifest: &str, version: &str) {
    let prefix = format!("manifest={manifest};module_version={version};pwsh=");
    let rest = value
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("config {value:?} must start with {prefix:?}"));
    let (pwsh, api) = rest.split_once(';').expect("config has an api part");
    assert_eq!(api, "api=dx.powershell.api/1");
    let parts: Vec<&str> = pwsh.split('.').collect();
    assert!(
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "pwsh version {pwsh:?} is not major.minor.patch"
    );
}

#[test]
fn the_ledger_shard_lists_exactly_the_exported_functions() {
    let shard = ledger();
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(shard.language, "powershell");
    assert_eq!(shard.package, "Ledger");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "powershell:Ledger:Add-LedgerEntry",
            "powershell:Ledger:Get-LedgerBalance",
            "powershell:Ledger:Remove-LedgerEntry",
            "powershell:Ledger:Test-LedgerBalance",
        ],
        "private and unexported functions must be absent"
    );
}

#[test]
fn a_function_with_two_parameter_sets_renders_both() {
    let shard = ledger();
    let add = symbol(&shard, "Add-LedgerEntry");
    assert_eq!(
        add.signature_text,
        "Add-LedgerEntry [-Ledger] <object> [-Amount] <int> [-Memo <string>] [<CommonParameters>]\n\
         Add-LedgerEntry [-Ledger] <object> [-Amount] <int> -Transfer -Account <string> [<CommonParameters>]"
    );
    assert_eq!(
        add.doc_markdown,
        "Adds one entry to a ledger.\n\nAppends the entry at the end of the ledger and returns it.\nAmounts are in the smallest currency unit."
    );
    let params: Vec<(&str, &str, &str)> = add
        .params
        .iter()
        .map(|p| (p.name.as_str(), p.r#type.as_str(), p.doc.as_str()))
        .collect();
    assert_eq!(
        params,
        [
            ("Ledger", "object", "The ledger to append to."),
            ("Amount", "int", "The amount in the smallest currency unit."),
            ("Memo", "string", "A free-text note stored with the entry."),
            (
                "Transfer",
                "switch",
                "Records the entry as a transfer between two accounts."
            ),
            ("Account", "string", "The counter account of a transfer."),
        ]
    );
    let returns = add.returns.as_ref().expect("returns");
    assert_eq!(returns.r#type, "pscustomobject");
    assert_eq!(returns.doc, "The entry that was added.");
    assert_eq!(
        add.examples,
        ["Add-LedgerEntry -Ledger $ledger -Amount 250\n\nAdds an entry of 250."]
    );
    let keys: Vec<&str> = add.extensions.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["powershell.aliases", "powershell.config"]);
    assert_eq!(extension(add, "powershell.aliases").as_deref(), Some("ale"));
}

#[test]
fn help_at_the_end_of_the_body_is_read() {
    let shard = ledger();
    let balance = symbol(&shard, "Get-LedgerBalance");
    assert_eq!(
        balance.signature_text,
        "Get-LedgerBalance [-Ledger] <object> [<CommonParameters>]"
    );
    assert_eq!(
        balance.doc_markdown,
        "Sums every entry of a ledger.\n\nThe help for this function sits at the end of its body."
    );
    assert_eq!(balance.params[0].doc, "The ledger to sum.");
    assert_eq!(
        balance.returns.as_ref().map(|r| r.r#type.as_str()),
        Some("int")
    );
    assert_eq!(extension(balance, "powershell.incomplete"), None);
}

#[test]
fn a_function_without_help_is_marked_incomplete() {
    let shard = ledger();
    let remove = symbol(&shard, "Remove-LedgerEntry");
    assert_eq!(
        remove.signature_text,
        "Remove-LedgerEntry [[-Ledger] <Object>] [[-Index] <Object>]"
    );
    assert_eq!(remove.doc_markdown, "");
    assert_eq!(remove.returns, None);
    assert_eq!(
        extension(remove, "powershell.incomplete").as_deref(),
        Some("help")
    );
}

#[test]
fn a_crlf_source_maps_to_lf_text() {
    let shard = ledger();
    let test = symbol(&shard, "Test-LedgerBalance");
    assert_eq!(
        test.doc_markdown,
        "Tells whether a ledger sums to zero.\n\nThis file uses CRLF line endings.\nThe description has two lines."
    );
    assert_eq!(
        test.signature_text,
        "Test-LedgerBalance [-Ledger] <object> [[-Tolerance] <int>] [<CommonParameters>]"
    );
    assert_eq!(
        extension(test, "powershell.incomplete").as_deref(),
        Some("params=Tolerance")
    );
    let source = std::fs::read(dx_testing::resolve_runfiles(&format!(
        "{LEDGER_DIR}/Public/Test-LedgerBalance.ps1"
    )))
    .expect("read the CRLF source");
    assert!(
        source.windows(2).any(|w| w == b"\r\n"),
        "the fixture must keep CRLF"
    );
}

#[test]
fn every_source_line_points_at_its_function_keyword() {
    for var in ["DX_LEDGER_SHARD", "DX_TALLY_SHARD"] {
        let (_, shard) = read_shard(var);
        for symbol in &shard.symbols {
            let source = symbol.source.as_ref().expect("a source");
            assert!(!source.generated);
            assert!(
                source.file.starts_with("tools/docs/powershell/fixture/"),
                "{} is not repo-relative",
                source.file
            );
            let text = std::fs::read_to_string(dx_testing::resolve_runfiles(&source.file))
                .expect("read the source");
            let name = symbol.id.rsplit(':').next().expect("a name");
            let line = text
                .lines()
                .nth(usize::try_from(source.line).expect("line fits") - 1)
                .expect("the line exists");
            assert_eq!(
                line.trim_end_matches('\r'),
                format!("function {name} {{"),
                "{}:{}",
                source.file,
                source.line
            );
        }
    }
}

#[test]
fn the_tally_shard_uses_literal_exports_and_its_own_config() {
    let (_, tally) = read_shard("DX_TALLY_SHARD");
    let ids: Vec<&str> = tally.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["powershell:Tally:Add-Tally"]);
    let add = &tally.symbols[0];
    assert_eq!(
        add.signature_text,
        "Add-Tally [[-Name] <string>] [[-Count] <int>]"
    );
    assert_eq!(add.examples, ["Add-Tally -Name votes -Count 2"]);
    let tally_config = extension(add, "powershell.config").expect("tally config");
    assert_config(&tally_config, "", "");
    let shard = ledger();
    let ledger_config =
        extension(symbol(&shard, "Add-LedgerEntry"), "powershell.config").expect("ledger config");
    assert_config(
        &ledger_config,
        &format!("{LEDGER_DIR}/Ledger.psd1"),
        "1.4.0",
    );
    assert_ne!(tally_config, ledger_config);
}

#[test]
fn re_encoding_a_shard_is_byte_identical() {
    for var in ["DX_LEDGER_SHARD", "DX_TALLY_SHARD"] {
        let (bytes, shard) = read_shard(var);
        assert_eq!(encode_shard(&shard).expect("encodes"), bytes, "{var}");
    }
}

fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn the_mapper_fails_without_writing_on_bad_api_json() {
    let mapper = runfile("DX_POWERSHELL_EXTRACT");
    let dir = scratch("bad_api");
    for (name, json) in [
        (
            "schema",
            r#"{"schema":"dx.powershell.api/9","pwsh":"7.5.4","files":[]}"#,
        ),
        ("malformed", "{"),
        (
            "dynamic",
            r#"{"schema":"dx.powershell.api/1","pwsh":"7.5.4","manifest":null,"files":[{"rel":"m.psm1","functions":[{"name":"Get-A","line":1,"help":null,"cmdlet_binding":null,"output_types":[],"aliases":[],"params":[]}],"exports":{"kind":"dynamic","functions":[]},"unresolved_uses":[]}]}"#,
        ),
        (
            "empty",
            r#"{"schema":"dx.powershell.api/1","pwsh":"7.5.4","manifest":null,"files":[]}"#,
        ),
    ] {
        let input = dir.join(format!("{name}.json"));
        let out = dir.join(format!("{name}.ir.pb"));
        std::fs::write(&input, json).expect("write the input");
        let run = Command::new(&mapper)
            .arg("--api")
            .arg(&input)
            .arg("--out")
            .arg(&out)
            .args(["--package", "Demo"])
            .output()
            .expect("run the mapper");
        assert_eq!(run.status.code(), Some(1), "{name}");
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(
            stderr.starts_with("powershell_extract: "),
            "{name}: {stderr}"
        );
        assert!(!out.exists(), "{name}: the mapper wrote {}", out.display());
    }
    let usage = Command::new(&mapper)
        .args(["--api", "x.json"])
        .output()
        .expect("run the mapper");
    assert_eq!(usage.status.code(), Some(2));
}
