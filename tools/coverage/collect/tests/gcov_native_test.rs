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

fn read_fixture(var: &str) -> Fixture {
    let text = fs::read_to_string(std::env::var(var).unwrap()).unwrap();
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

impl Run {
    fn manifest_entries(&self) -> Vec<String> {
        fs::read_to_string(&self.manifest)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

fn setup(name: &str) -> Run {
    let fixture = read_fixture("GCOV_FIXTURE");
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
fn reports_shared_library_lines() {
    let run = setup("shared");
    let out = collect(&run);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lcov = fs::read_to_string(run.coverage_dir.join("coverage.dat")).unwrap();
    let record = lcov
        .split("end_of_record")
        .find(|r| r.contains("SF:cc/tests/fixtures/gcc_shared/greet.cc"))
        .unwrap_or_else(|| panic!("{lcov}"));
    let hit = record
        .lines()
        .filter_map(|l| {
            l.strip_prefix("DA:")?
                .split_once(',')?
                .1
                .parse::<u64>()
                .ok()
        })
        .any(|k| k > 0);
    assert!(hit, "{record}");
}

fn line_counts(lcov: &str, source: &str) -> Vec<(u64, u64)> {
    let record = lcov
        .split("end_of_record")
        .find(|r| r.contains(&format!("SF:{source}")))
        .unwrap_or_else(|| panic!("{lcov}"));
    record
        .lines()
        .filter_map(|l| {
            let (line, count) = l.strip_prefix("DA:")?.split_once(',')?;
            Some((line.parse().ok()?, count.parse().ok()?))
        })
        .collect()
}

#[test]
fn merged_runs_add_hit_counts() {
    let source = "cc/tests/fixtures/gcc_coverage/lib.c";
    let merge_dir = Path::new(&std::env::var("TEST_TMPDIR").unwrap()).join("merged");
    fs::create_dir_all(&merge_dir).unwrap();
    let mut single = Vec::new();
    for name in ["first", "second"] {
        let run = setup(name);
        let out = collect(&run);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let lcov = fs::read_to_string(run.coverage_dir.join("coverage.dat")).unwrap();
        fs::write(merge_dir.join(format!("{name}.dat")), &lcov).unwrap();
        single = line_counts(&lcov, source);
    }
    assert!(single.iter().any(|&(_, k)| k > 0));
    assert!(single.iter().any(|&(_, k)| k == 0));
    let output = merge_dir.join("merged.out");
    let status = Command::new(absolute(&std::env::var("LCOV_MERGER").unwrap()))
        .arg(format!("--coverage_dir={}", merge_dir.display()))
        .arg(format!("--output_file={}", output.display()))
        .status()
        .unwrap();
    assert!(status.success());
    let merged = line_counts(&fs::read_to_string(&output).unwrap(), source);
    let doubled: Vec<(u64, u64)> = single.iter().map(|&(l, k)| (l, k * 2)).collect();
    assert_eq!(merged, doubled);
}

#[test]
fn fails_on_stale_gcda() {
    let run = setup("stale");
    let alt = read_fixture("GCOV_FIXTURE_ALT");
    let mut replaced = false;
    for pair in &alt.notes {
        let (alt_path, source) = pair.split_once('=').unwrap();
        let name = Path::new(alt_path).file_name().unwrap();
        if name != "lib.pic.gcno" {
            continue;
        }
        let dest = run
            .manifest_entries()
            .into_iter()
            .find(|p| Path::new(p).file_name() == Some(name))
            .unwrap();
        let dest = run.root.join(dest);
        fs::remove_file(&dest).unwrap();
        fs::copy(absolute(source), dest).unwrap();
        replaced = true;
    }
    assert!(replaced);
    let out = collect(&run);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("lib.pic.gcda"), "{stderr}");
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
