use std::collections::BTreeSet;
use std::path::Path;

const ALLOWLIST: &str = "tools/testing/host_path_allowlist.txt";
const SELF: &str = "tools/testing/host_path_scan_test.rs";
const EXTENSIONS: &[&str] = &["bzl", "rs", "sh", "tpl"];

fn is_skipped_dir(name: &str) -> bool {
    name == ".git" || name == "node_modules" || name == "target" || name.starts_with("bazel-")
}

fn is_scanned(name: &str) -> bool {
    name.ends_with("BUILD.bazel")
        || Path::new(name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| EXTENSIONS.contains(&extension))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{dir:?}: {error}"));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("{dir:?}: {error}"));
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let kind = entry
            .file_type()
            .unwrap_or_else(|error| panic!("{path:?}: {error}"));
        if kind.is_dir() {
            if !is_skipped_dir(&name) {
                collect(root, &path, out);
            }
        } else if kind.is_file() && is_scanned(&name) {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn is_shell_shebang(first_line: &str) -> bool {
    let Some(rest) = first_line.strip_prefix("#!") else {
        return false;
    };
    rest.split_whitespace()
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .any(|word| matches!(word, "sh" | "bash" | "zsh" | "dash" | "ksh"))
}

fn hits(rel: &str, text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    if text.contains(concat!("/usr/bin", ":/bin")) {
        found.push("host PATH");
    }
    if text.contains(concat!("use_default_shell_env", " = True")) {
        found.push("use_default_shell_env");
    }
    if text.contains(concat!("sh_test", "(")) && !rel.contains("/tests/fixtures/") {
        found.push("sh_test");
    }
    if text.contains(concat!("run_shell", "(")) {
        found.push("run_shell");
    }
    if text.lines().next().is_some_and(is_shell_shebang) {
        found.push("shell shebang");
    }
    found
}

fn allowlist(root: &Path) -> BTreeSet<String> {
    let path = root.join(ALLOWLIST);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn host_path_sources_match_the_allowlist() {
    let root = dx_testing::rules_dx_source();
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let mut flagged = BTreeSet::new();
    for rel in files.iter().filter(|rel| rel.as_str() != SELF) {
        let bytes = std::fs::read(root.join(rel)).unwrap_or_else(|error| panic!("{rel}: {error}"));
        let text = String::from_utf8_lossy(&bytes);
        let found = hits(rel, &text);
        if !found.is_empty() {
            flagged.insert(rel.clone());
        }
    }
    let allowed = allowlist(&root);
    let new: Vec<&String> = flagged.difference(&allowed).collect();
    let stale: Vec<&String> = allowed.difference(&flagged).collect();
    assert!(
        new.is_empty(),
        "files using host PATH, use_default_shell_env, sh_test, run_shell or a shell shebang: {new:?}"
    );
    assert!(
        stale.is_empty(),
        "{ALLOWLIST} entries with no hit; delete them: {stale:?}"
    );
}
