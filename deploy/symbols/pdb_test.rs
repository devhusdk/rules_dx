use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn entry(var: &str, suffix: &str) -> PathBuf {
    let list = std::env::var(var).unwrap_or_else(|_| panic!("{var} must list the symbols"));
    let rel = list
        .split_whitespace()
        .find(|p| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("{var} has no {suffix}"));
    dx_testing::resolve_runfiles(rel)
}

fn known_pdb() -> PathBuf {
    entry("DX_KNOWN_PDB_SYMBOLS", ".pdb")
}

fn known_manifest() -> PathBuf {
    entry("DX_KNOWN_PDB_SYMBOLS", ".pdb.json")
}

fn other_pdb() -> PathBuf {
    entry("DX_OTHER_PDB_SYMBOLS", ".pdb")
}

fn str_of(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn symbolizes_known_function_from_the_pdb() {
    let tmp = dx_testing::mkscratch("pdb-symbolize").unwrap();
    let exe = tmp.join("known.exe");
    let pdb = tmp.join("known.pdb");
    std::fs::copy(data("DX_KNOWN_BINARY"), &exe).unwrap();
    std::fs::copy(known_pdb(), &pdb).unwrap();
    let run = dx_testing::run(&exe, &[], &[]).unwrap();
    let rva: i64 = run.stdout.lines().nth(1).unwrap().trim().parse().unwrap();
    let obj = format!("--obj={}", exe.display());
    let pdb_arg = format!("--pdb={}", pdb.display());
    let address = format!("0x{rva:x}");
    let sym = dx_testing::run(
        &data("DX_LLVM_SYMBOLIZER"),
        &[
            obj.as_str(),
            pdb_arg.as_str(),
            "--relative-address",
            address.as_str(),
        ],
        &[],
    )
    .unwrap();
    assert!(sym.status.success(), "{}", sym.stderr);
    let lines: Vec<&str> = sym.stdout.lines().collect();
    assert_eq!(lines[0], "dx_known_function", "{}", sym.stdout);
    let parts: Vec<&str> = lines[1].split(':').collect();
    assert!(parts.len() >= 3, "{}", lines[1]);
    let file = parts[parts.len() - 3];
    let line: usize = parts[parts.len() - 2].parse().unwrap();
    assert!(file.ends_with("known.c"), "{file}");
    let source = String::from_utf8(read(&data("DX_KNOWN_SOURCE"))).unwrap();
    let text = source.lines().nth(line - 1).unwrap_or("");
    assert!(
        text.contains("dx_known_function") || text.contains("x * 7"),
        "line {line} is {text:?}"
    );
}

#[test]
fn manifest_matches_the_binary_and_its_pdb() {
    let binary = data("DX_KNOWN_BINARY");
    let pe = dx_symbols::pe_codeview(&read(&binary)).unwrap();
    let json = dx_testing::read_json(&known_manifest()).unwrap();
    let guid: String = pe.guid.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(json["guid"], guid);
    assert_eq!(json["age"], pe.age);
    assert_eq!(
        json["sha256"].as_str().unwrap(),
        dx_testing::sha256_file(&binary).unwrap()
    );
}

#[test]
fn codeview_path_is_a_bare_file_name() {
    let pe = dx_symbols::pe_codeview(&read(&data("DX_KNOWN_BINARY"))).unwrap();
    assert!(
        !pe.path.contains('/') && !pe.path.contains('\\'),
        "{}",
        pe.path
    );
}

fn fails_and_writes_nothing(name: &str, pdb: Option<&Path>) {
    let tmp = dx_testing::mkscratch(name).unwrap();
    let manifest = tmp.join("manifest.json");
    let binary = data("DX_KNOWN_BINARY");
    let mut args = vec!["--in", str_of(&binary), "--manifest", str_of(&manifest)];
    if let Some(pdb) = pdb {
        args.extend(["--pdb", str_of(pdb)]);
    }
    let run = dx_testing::run(&data("DX_PDB"), &args, &[]).unwrap();
    assert!(!run.status.success(), "{}", run.stdout);
    assert!(!manifest.exists());
}

#[test]
fn pdb_of_another_binary_fails() {
    fails_and_writes_nothing("pdb-other", Some(&other_pdb()));
}

#[test]
fn truncated_pdb_fails() {
    let tmp = dx_testing::mkscratch("pdb-truncated").unwrap();
    let bytes = read(&known_pdb());
    let short = tmp.join("short.pdb");
    std::fs::write(&short, &bytes[..bytes.len() / 2]).unwrap();
    fails_and_writes_nothing("pdb-truncated-run", Some(&short));
}

#[test]
fn missing_pdb_flag_fails() {
    fails_and_writes_nothing("pdb-missing", None);
}
