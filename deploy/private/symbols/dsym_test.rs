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

fn known_bundle() -> PathBuf {
    entry("DX_KNOWN_DSYM_SYMBOLS", ".dSYM")
}

fn known_manifest() -> PathBuf {
    entry("DX_KNOWN_DSYM_SYMBOLS", ".dsym.json")
}

#[test]
fn symbolizes_known_function_from_the_dsym_alone() {
    let dwarf = dx_symbols::dwarf_file(&known_bundle()).expect("one DWARF file in the dSYM");
    let nm = dx_testing::run(&data("DX_LLVM_NM"), &[dwarf.to_str().unwrap()], &[]).unwrap();
    let addr = nm
        .stdout
        .lines()
        .find_map(|l| {
            let mut words = l.split_whitespace();
            let addr = words.next()?;
            words
                .any(|w| w.trim_start_matches('_') == "dx_known_function")
                .then(|| addr.to_string())
        })
        .expect("dx_known_function in the dSYM");
    let obj = format!("--obj={}", dwarf.display());
    let address = format!("0x{addr}");
    let sym = dx_testing::run(
        &data("DX_LLVM_SYMBOLIZER"),
        &[obj.as_str(), address.as_str()],
        &[],
    )
    .unwrap();
    assert!(sym.status.success(), "{}", sym.stderr);
    let lines: Vec<&str> = sym.stdout.lines().collect();
    assert_eq!(lines[0].trim_start_matches('_'), "dx_known_function");
    let parts: Vec<&str> = lines[1].split(':').collect();
    assert!(parts.len() >= 3, "{}", lines[1]);
    let file = parts[parts.len() - 3];
    let line: usize = parts[parts.len() - 2].parse().unwrap();
    assert!(
        file.ends_with("known.c") && !file.starts_with('/'),
        "{file}"
    );
    assert!(line > 0, "{}", sym.stdout);
    let source = String::from_utf8(read(&data("DX_KNOWN_SOURCE"))).unwrap();
    let text = source.lines().nth(line - 1).unwrap_or("");
    assert!(
        text.contains("dx_known_function") || text.contains("x * 7"),
        "line {line} is {text:?}"
    );
}

#[test]
fn manifest_matches_the_binary_and_its_dsym() {
    let binary = data("DX_KNOWN_BINARY");
    let dwarf = dx_symbols::dwarf_file(&known_bundle()).unwrap();
    let pair = dx_symbols::check_dsym(&read(&binary), &read(&dwarf)).unwrap();
    let json = dx_testing::read_json(&known_manifest()).unwrap();
    assert_eq!(json["uuid"], pair.uuid);
    assert_eq!(json["arch"], pair.arch);
    assert!(
        json["dwarf"]
            .as_str()
            .unwrap()
            .starts_with("Contents/Resources/DWARF/")
    );
    assert!(!json["dsymutil"].as_str().unwrap_or("").is_empty());
    assert_eq!(
        json["sha256"].as_str().unwrap(),
        dx_testing::sha256_file(&binary).unwrap()
    );
}

#[test]
fn missing_dsym_fails_and_writes_nothing() {
    let tmp = dx_testing::mkscratch("dsym-missing").unwrap();
    let manifest = tmp.join("manifest.json");
    let missing = tmp.join("missing.dSYM");
    let dsymutil = data("DX_DSYMUTIL");
    let binary = data("DX_KNOWN_BINARY");
    let args = [
        "--dsymutil",
        dsymutil.to_str().unwrap(),
        "--in",
        binary.to_str().unwrap(),
        "--bundle",
        missing.to_str().unwrap(),
        "--manifest",
        manifest.to_str().unwrap(),
    ];
    let run = dx_testing::run(&data("DX_DSYM"), &args, &[]).unwrap();
    assert!(!run.status.success());
    assert!(run.stderr.contains("cannot read dSYM"), "{}", run.stderr);
    assert!(!manifest.exists());
}
