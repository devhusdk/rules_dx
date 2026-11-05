//! Runs collect_native on a binary built with GCC coverage instrumentation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture {
    tests: Vec<PathBuf>,
    notes: Vec<String>,
    gcov: PathBuf,
}

fn absolute(path: &str) -> PathBuf {
    std::env::current_dir().unwrap().join(path)
}

fn read_fixture() -> Fixture {
    let text = fs::read_to_string(std::env::var("GCOV_FIXTURE").unwrap()).unwrap();
    let mut fixture = Fixture {
        tests: Vec::new(),
        notes: Vec::new(),
        gcov: PathBuf::new(),
    };
    for line in text.lines() {
        match line.split_once('=').unwrap() {
            ("test", path) => fixture.tests.push(absolute(path)),
            ("gcno", pair) => fixture.notes.push(pair.to_string()),
            ("gcov", path) => fixture.gcov = absolute(path),
            (kind, _) => panic!("unknown fixture line {kind}"),
        }
    }
    fixture
}

fn gcda_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for path in fs::read_dir(dir).unwrap().flatten().map(|e| e.path()) {
        if path.is_dir() {
            gcda_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "gcda") {
            found.push(path);
        }
    }
}

struct Run {
    root: PathBuf,
    coverage_dir: PathBuf,
    manifest: PathBuf,
    gcov: PathBuf,
}

fn setup(name: &str) -> Run {
    let fixture = read_fixture();
    let base = Path::new(&std::env::var("TEST_TMPDIR").unwrap()).join(name);
    let root = base.join("root");
    let coverage_dir = base.join("cov");
    fs::create_dir_all(&coverage_dir).unwrap();
    let mut manifest = String::new();
    for pair in &fixture.notes {
        let (path, source) = pair.split_once('=').unwrap();
        let dest = root.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(absolute(source), dest).unwrap();
        manifest.push_str(path);
        manifest.push('\n');
    }
    let manifest_path = base.join("manifest.txt");
    fs::write(&manifest_path, manifest).unwrap();
    for test in &fixture.tests {
        let status = Command::new(test)
            .env("GCOV_PREFIX", &coverage_dir)
            .status()
            .unwrap();
        assert!(status.success(), "{} failed", test.display());
    }
    Run {
        root,
        coverage_dir,
        manifest: manifest_path,
        gcov: fixture.gcov,
    }
}

fn collect(run: &Run) -> Output {
    let execroot = run.root.parent().unwrap().join("execroot");
    let _ = fs::create_dir_all(&execroot);
    let _ = std::os::unix::fs::symlink(
        std::env::var("TEST_SRCDIR").unwrap(),
        execroot.join("external"),
    );
    Command::new(absolute(&std::env::var("COLLECT_NATIVE").unwrap()))
        .env("EXECROOT", execroot)
        .env("GCOV", &run.gcov)
        .env("ROOT", &run.root)
        .env("COVERAGE_DIR", &run.coverage_dir)
        .env("COVERAGE_MANIFEST", &run.manifest)
        .output()
        .unwrap()
}

#[test]
fn reports_hit_and_missed_lines() {
    let run = setup("report");
    let out = collect(&run);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lcov = fs::read_to_string(run.coverage_dir.join("coverage.dat")).unwrap();
    assert!(
        lcov.contains("SF:cc/tests/fixtures/gcc_coverage/lib.c"),
        "{lcov}"
    );
    let counts: Vec<u64> = lcov
        .lines()
        .filter_map(|l| l.strip_prefix("DA:")?.split_once(',')?.1.parse().ok())
        .collect();
    assert!(counts.iter().any(|&k| k > 0), "{lcov}");
    assert!(counts.contains(&0), "{lcov}");
}

#[test]
fn fails_on_wrong_gcda_version() {
    let run = setup("corrupt");
    let mut gcdas = Vec::new();
    gcda_files(&run.coverage_dir, &mut gcdas);
    assert!(!gcdas.is_empty());
    for gcda in &gcdas {
        let mut bytes = fs::read(gcda).unwrap();
        bytes[4..8].copy_from_slice(b"0000");
        fs::write(gcda, bytes).unwrap();
    }
    let out = collect(&run);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let name = gcdas
        .iter()
        .map(|g| g.file_name().unwrap().to_string_lossy())
        .find(|n| stderr.contains(n.as_ref()));
    assert!(name.is_some(), "{stderr}");
}
