use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

fn symbols(var: &str, suffix: &str) -> PathBuf {
    let list = std::env::var(var).unwrap_or_else(|_| panic!("{var} must list the symbols"));
    let rel = list
        .split_whitespace()
        .find(|p| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("{var} has no {suffix}"));
    dx_testing::resolve_runfiles(rel)
}

fn known_split() -> (PathBuf, PathBuf, PathBuf) {
    (
        data("DX_KNOWN_STRIPPED"),
        symbols("DX_KNOWN_SYMBOLS", ".debug"),
        symbols("DX_KNOWN_SYMBOLS", ".symbols.json"),
    )
}

#[test]
fn symbolizes_known_function_from_debug_file() {
    let (_, debug, _) = known_split();
    let tmp = dx_testing::mkscratch("symbols").unwrap();
    let copy = tmp.join("known.debug");
    std::fs::copy(&debug, &copy).unwrap();
    let nm = dx_testing::run(&data("DX_LLVM_NM"), &[copy.to_str().unwrap()], &[]).unwrap();
    let addr = nm
        .stdout
        .lines()
        .find(|l| l.ends_with(" dx_known_function"))
        .and_then(|l| l.split_whitespace().next())
        .expect("dx_known_function in the debug file");
    let sym = dx_testing::run(
        &data("DX_LLVM_SYMBOLIZER"),
        &[&format!("--obj={}", copy.display()), &format!("0x{addr}")],
        &[],
    )
    .unwrap();
    assert!(sym.status.success(), "{}", sym.stderr);
    let lines: Vec<&str> = sym.stdout.lines().collect();
    assert_eq!(lines[0], "dx_known_function");
    let parts: Vec<&str> = lines[1].split(':').collect();
    assert!(parts.len() >= 3, "{}", lines[1]);
    let file = parts[parts.len() - 3];
    let line = parts[parts.len() - 2];
    assert!(
        file.ends_with("known.c") && !file.starts_with('/'),
        "{file}"
    );
    let line: usize = line.parse().unwrap();
    assert!(line > 0, "{}", sym.stdout);
    let source = String::from_utf8(read(&data("DX_KNOWN_SOURCE"))).unwrap();
    let text = source.lines().nth(line - 1).unwrap_or("");
    assert!(
        text.contains("dx_known_function") || text.contains("x * 7"),
        "line {line} is {text:?}"
    );
}

#[test]
fn stripped_binary_has_no_debug_info_and_runs_the_same() {
    let (stripped, _, _) = known_split();
    let bytes = read(&stripped);
    assert!(!contains(&bytes, ".debug_info"));
    assert!(!contains(&bytes, ".symtab"));
    let a = dx_testing::run(&stripped, &[], &[]).unwrap();
    let b = dx_testing::run(&data("DX_KNOWN_BINARY"), &[], &[]).unwrap();
    assert_eq!(a.stdout, b.stdout);
    assert_eq!(a.status.code(), b.status.code());
}

#[test]
fn manifest_matches_the_binary() {
    let (stripped, debug, manifest) = known_split();
    let pair = dx_symbols::check_pair(&read(&stripped), &read(&debug)).unwrap();
    let json = dx_testing::read_json(&manifest).unwrap();
    assert_eq!(json["build_id"], pair.build_id);
    assert_eq!(json["arch"], pair.arch);
    assert_eq!(
        json["sha256"].as_str().unwrap(),
        dx_testing::sha256_file(&stripped).unwrap()
    );
}

#[test]
fn split_without_build_id_writes_nothing() {
    let tmp = dx_testing::mkscratch("symbols-nobuildid").unwrap();
    let out = |n: &str| tmp.join(n).to_str().unwrap().to_string();
    let args = [
        "--objcopy".to_string(),
        data("DX_LLVM_OBJCOPY").to_str().unwrap().to_string(),
        "--in".to_string(),
        data("DX_NO_BUILD_ID").to_str().unwrap().to_string(),
        "--stripped".to_string(),
        out("stripped"),
        "--debug".to_string(),
        out("debug"),
        "--manifest".to_string(),
        out("manifest"),
    ];
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let run = dx_testing::run(&data("DX_SPLIT"), &refs, &[]).unwrap();
    assert!(!run.status.success());
    assert!(run.stderr.contains("build-id"), "{}", run.stderr);
    for n in ["stripped", "debug", "manifest"] {
        assert!(!tmp.join(n).exists(), "{n} was written");
    }
}

#[test]
fn split_missing_input_fails() {
    let tmp = dx_testing::mkscratch("symbols-missing").unwrap();
    let missing = tmp.join("missing");
    let out = |n: &str| tmp.join(n).to_str().unwrap().to_string();
    let objcopy = data("DX_LLVM_OBJCOPY");
    let args = [
        "--objcopy",
        objcopy.to_str().unwrap(),
        "--in",
        missing.to_str().unwrap(),
        "--stripped",
        &out("stripped"),
        "--debug",
        &out("debug"),
        "--manifest",
        &out("manifest"),
    ];
    let run = dx_testing::run(&data("DX_SPLIT"), &args, &[]).unwrap();
    assert!(!run.status.success());
}

#[test]
fn check_pair_rejects_bad_pairs() {
    let (stripped, debug, _) = known_split();
    let stripped = read(&stripped);
    let debug = read(&debug);
    let other = read(&symbols("DX_OTHER_SYMBOLS", ".debug"));
    let err = dx_symbols::check_pair(&stripped, &other).unwrap_err();
    assert!(err.contains("build-id mismatch"), "{err}");
    assert!(dx_symbols::check_pair(&stripped, &debug[..64]).is_err());
    let mut patched = debug.clone();
    patched[18] = 0x3e ^ 0x28;
    patched[19] = 0;
    assert!(dx_symbols::check_pair(&stripped, &patched).is_err());
}
