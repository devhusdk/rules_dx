use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source dir") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn raw_current_dir_reads(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let cut = lines
        .iter()
        .enumerate()
        .find(|(index, line)| {
            line.trim() == "#[cfg(test)]"
                && lines[index + 1..]
                    .iter()
                    .find(|next| !next.trim().is_empty())
                    .is_some_and(|next| next.trim_start().starts_with("mod "))
        })
        .map_or(lines.len(), |(index, _)| index);
    lines[..cut]
        .iter()
        .filter(|line| line.contains("env::current_dir"))
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn raw_current_dir_reads_stay_out_of_cli_production_code() {
    let mut sources = Vec::new();
    rust_sources(&workspace_root().join("cli/cli/src"), &mut sources);
    assert!(
        sources.iter().any(|path| path.ends_with("exec/common.rs")),
        "cli/cli/src sources are not in the test data"
    );
    let mut hits = Vec::new();
    for path in sources {
        let file = path
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        if file.ends_with("_tests.rs") || file == "test_support.rs" {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("source");
        for line in raw_current_dir_reads(&source) {
            hits.push(format!("{}: {line}", path.display()));
        }
    }
    assert!(
        hits.is_empty(),
        "use dx_process::current_directory() instead of std::env::current_dir(): {hits:?}"
    );
}

#[test]
fn current_dir_scanner_flags_raw_reads_and_skips_test_modules() {
    let source = "fn a() {\n    let cwd = std::env::current_dir();\n}\n\
                  #[cfg(test)]\nmod tests {\n    fn t() { std::env::current_dir(); }\n}\n";
    let hits = raw_current_dir_reads(source);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].contains("let cwd"), "{hits:?}");
    assert!(
        raw_current_dir_reads("let cwd = dx_process::current_directory();\n").is_empty(),
        "the dx_process wrapper must not be flagged"
    );
}
