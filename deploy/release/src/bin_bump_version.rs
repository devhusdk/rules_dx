#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const USAGE: &str = "usage: bazel run //deploy/release:bump_version -- <version> [--workflow-ref <commit>]\n\nSets the rules_dx version and the starter workflow pin in every file that carries them, including each families/*/MODULE.bazel, and refreshes tools/ci/starter_pin.sha256 from the workflows in the checkout.\n--workflow-ref defaults to origin/main and must have the affected_on_pr input.";
const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";
const DIGEST_RECORD: &str = "tools/ci/starter_pin.sha256";

struct Edit {
    marker: &'static str,
    after: Option<&'static str>,
    pin: bool,
}

const fn version(marker: &'static str) -> Edit {
    Edit {
        marker,
        after: None,
        pin: false,
    }
}

const fn pin(marker: &'static str) -> Edit {
    Edit {
        marker,
        after: None,
        pin: true,
    }
}

const MODULE_NAME: &str = "name = \"rules_dx\"";

fn targets() -> Vec<(&'static str, Vec<Edit>)> {
    vec![
        (
            "MODULE.bazel",
            vec![Edit {
                marker: "    version = \"",
                after: Some(MODULE_NAME),
                pin: false,
            }],
        ),
        ("cli/prebuilt/pins.bzl", vec![version("DX_VERSION = \"")]),
        (
            "cli/adopt/src/version.rs",
            vec![
                version("DX_VERSION: &str = \""),
                version("MODULE_VERSION: &str = \""),
            ],
        ),
        (
            "cli/adopt/src/scaffold.rs",
            vec![pin("CONSUMER_WORKFLOW_PIN: &str = \"")],
        ),
        (
            "examples/consumer-ci/caller.yml",
            vec![pin("reusable-consumer.yml@")],
        ),
        (
            "examples/docs-ci/caller.yml",
            vec![pin("reusable-docs.yml@")],
        ),
        (
            "examples/gitlab-ci/.gitlab-ci.yml",
            vec![pin("githubusercontent.com/devhusdk/rules_dx/")],
        ),
    ]
}

fn family_targets(root: &Path) -> Result<Vec<(String, Vec<Edit>)>, String> {
    let dir = root.join("families");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("families: {e}"))? {
        let family = entry.map_err(|e| format!("families: {e}"))?.file_name();
        let file = format!("families/{}/MODULE.bazel", family.to_string_lossy());
        if root.join(&file).is_file() {
            files.push(file);
        }
    }
    files.sort();
    Ok(files
        .into_iter()
        .map(|file| {
            let edits = vec![
                Edit {
                    marker: "    version = \"",
                    after: Some("name = \"rules_dx_"),
                    pin: false,
                },
                version("bazel_dep(name = \"rules_dx\", version = \""),
            ];
            (file, edits)
        })
        .collect())
}

fn valid_version(text: &str) -> bool {
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (text, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let number = |part: &&str| {
        !part.is_empty()
            && part.chars().all(|c| c.is_ascii_digit())
            && (*part == "0" || !part.starts_with('0'))
    };
    parts.len() == 3
        && parts.iter().all(number)
        && pre.is_none_or(|pre| {
            !pre.is_empty()
                && pre
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        })
}

fn valid_commit(text: &str) -> bool {
    text.len() == 40 && text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn rewrite(text: &str, edit: &Edit, value: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut previous = "";
    let mut hits = 0;
    for line in text.split_inclusive('\n') {
        let allowed = edit.after.is_none_or(|after| previous.contains(after));
        match line.find(edit.marker).filter(|_| allowed) {
            Some(at) => {
                let start = at + edit.marker.len();
                let end = line[start..]
                    .find(|c: char| matches!(c, '"' | '/') || c.is_whitespace())
                    .map_or(line.len(), |n| start + n);
                out.push_str(&line[..start]);
                out.push_str(value);
                out.push_str(&line[end..]);
                hits += 1;
            }
            None => out.push_str(line),
        }
        previous = line;
    }
    if hits == 0 {
        return Err(format!("no line with `{}`", edit.marker));
    }
    Ok(out)
}

fn redigest(root: &Path, record: &str) -> Result<String, String> {
    let mut out = String::with_capacity(record.len());
    for line in record.lines() {
        let (_, path) = line
            .split_once("  ")
            .ok_or_else(|| format!("malformed line `{line}`"))?;
        let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        out.push_str(&format!("{}  {path}\n", dx_digest::sha256_hex(&bytes)));
    }
    Ok(out)
}

fn bump(root: &Path, version: &str, commit: &str) -> Result<usize, String> {
    if !valid_version(version) {
        return Err(format!("`{version}` is not a version like 1.2.3"));
    }
    if !valid_commit(commit) {
        return Err(format!("`{commit}` is not a 40 character commit"));
    }
    let mut changed = Vec::new();
    let mut all: Vec<(String, Vec<Edit>)> = targets()
        .into_iter()
        .map(|(file, edits)| (file.to_owned(), edits))
        .collect();
    all.extend(family_targets(root)?);
    for (file, edits) in all {
        let path = root.join(&file);
        let old = fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
        let mut text = old.clone();
        for edit in &edits {
            let value = if edit.pin { commit } else { version };
            text = rewrite(&text, edit, value).map_err(|e| format!("{file}: {e}"))?;
        }
        if text != old {
            changed.push((path, text));
        }
    }
    let record = root.join(DIGEST_RECORD);
    let old = fs::read_to_string(&record).map_err(|e| format!("{DIGEST_RECORD}: {e}"))?;
    let text = redigest(root, &old).map_err(|e| format!("{DIGEST_RECORD}: {e}"))?;
    if text != old {
        changed.push((record, text));
    }
    for (path, text) in &changed {
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(changed.len())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn has_affected_on_pr(workflow: &str) -> bool {
    workflow
        .lines()
        .any(|line| line.trim_start().starts_with("affected_on_pr:"))
}

fn resolve_commit(root: &Path, reference: &str) -> Result<String, String> {
    let commit = git(root, &["rev-parse", &format!("{reference}^{{commit}}")])?;
    let workflow = git(root, &["show", &format!("{commit}:{CONSUMER_WORKFLOW}")])?;
    if !has_affected_on_pr(&workflow) {
        return Err(format!(
            "{commit} has no affected_on_pr input in {CONSUMER_WORKFLOW}"
        ));
    }
    Ok(commit)
}

fn run(args: &[String], workspace: Option<PathBuf>) -> Result<String, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(USAGE.to_owned());
    }
    let mut version = None;
    let mut reference = "origin/main".to_owned();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workflow-ref" => {
                reference = iter
                    .next()
                    .ok_or_else(|| "--workflow-ref needs a commit".to_owned())?
                    .clone();
            }
            other if !other.starts_with('-') && version.is_none() => version = Some(other),
            other => return Err(format!("unexpected argument `{other}`\n{USAGE}")),
        }
    }
    let version = version.ok_or_else(|| USAGE.to_owned())?;
    let root = workspace
        .filter(|path| path.is_absolute())
        .ok_or("BUILD_WORKSPACE_DIRECTORY must be an absolute path (run with bazel run)")?;
    if !valid_version(version) {
        return Err(format!("`{version}` is not a version like 1.2.3"));
    }
    let commit = resolve_commit(&root, &reference)?;
    let count = bump(&root, version, &commit)?;
    Ok(format!(
        "bump_version: {count} files changed, version {version}, workflow pin {commit}"
    ))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY").map(PathBuf::from);
    match run(&args, workspace) {
        Ok(message) => println!("{message}"),
        Err(message) => {
            eprintln!("bump_version: {message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "e97ab7b550d08e047c35edc5f50c7d242227842f";
    const NEW: &str = "0123456789abcdef0123456789abcdef01234567";

    fn fixture(root: &Path) {
        let files = [
            (
                "MODULE.bazel",
                "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\nbazel_dep(name = \"x\", version = \"9.9.9\")\n",
            ),
            ("cli/prebuilt/pins.bzl", "DX_VERSION = \"0.0.0\"\n"),
            (
                "cli/adopt/src/version.rs",
                "pub const DX_VERSION: &str = \"0.0.0\";\npub const MODULE_VERSION: &str = \"0.0.0\";\n",
            ),
            (
                "cli/adopt/src/scaffold.rs",
                "pub const CONSUMER_WORKFLOW_PIN: &str = \"e97ab7b550d08e047c35edc5f50c7d242227842f\";\n",
            ),
            (
                ".github/workflows/reusable-docs.yml",
                "name: docs\n",
            ),
            (
                "tools/ci/starter_pin.sha256",
                "0000000000000000000000000000000000000000000000000000000000000000  .github/workflows/reusable-docs.yml\n",
            ),
            (
                "examples/consumer-ci/caller.yml",
                "    uses: devhusdk/rules_dx/.github/workflows/reusable-consumer.yml@e97ab7b550d08e047c35edc5f50c7d242227842f\n    with:\n",
            ),
            (
                "examples/gitlab-ci/.gitlab-ci.yml",
                "include:\n  - remote: https://raw.githubusercontent.com/devhusdk/rules_dx/e97ab7b550d08e047c35edc5f50c7d242227842f/ci/gitlab/dx.gitlab-ci.yml\n",
            ),
            (
                "examples/docs-ci/caller.yml",
                "    uses: devhusdk/rules_dx/.github/workflows/reusable-docs.yml@e97ab7b550d08e047c35edc5f50c7d242227842f\n    with:\n      docs_scope: \"//...\"\n",
            ),
        ];
        for (file, text) in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    fn read(root: &Path, file: &str) -> String {
        fs::read_to_string(root.join(file)).unwrap()
    }

    #[test]
    fn rewrites_every_file() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        assert_eq!(bump(dir.path(), "1.2.3", NEW), Ok(8));
        assert!(read(dir.path(), "MODULE.bazel").contains("    version = \"1.2.3\","));
        assert!(read(dir.path(), "MODULE.bazel").contains("name = \"x\", version = \"9.9.9\""));
        assert_eq!(
            read(dir.path(), "cli/prebuilt/pins.bzl"),
            "DX_VERSION = \"1.2.3\"\n"
        );
        let version = read(dir.path(), "cli/adopt/src/version.rs");
        assert!(version.contains("DX_VERSION: &str = \"1.2.3\";"));
        assert!(version.contains("MODULE_VERSION: &str = \"1.2.3\";"));
        assert!(read(dir.path(), "cli/adopt/src/scaffold.rs").contains(&format!("\"{NEW}\";")));
        assert_eq!(
            read(dir.path(), "tools/ci/starter_pin.sha256"),
            format!(
                "{}  .github/workflows/reusable-docs.yml\n",
                dx_digest::sha256_hex(b"name: docs\n")
            )
        );
        let consumer = read(dir.path(), "examples/consumer-ci/caller.yml");
        assert!(consumer.contains(&format!("reusable-consumer.yml@{NEW}\n")));
        assert!(!consumer.contains("rules_dx_version"));
        let docs_ci = read(dir.path(), "examples/docs-ci/caller.yml");
        assert!(docs_ci.contains(&format!("reusable-docs.yml@{NEW}\n")));
        assert!(docs_ci.contains("docs_scope: \"//...\""));
        assert!(!docs_ci.contains(OLD));
        assert!(
            read(dir.path(), "examples/gitlab-ci/.gitlab-ci.yml")
                .contains(&format!("rules_dx/{NEW}/ci/gitlab/dx.gitlab-ci.yml\n"))
        );
    }

    #[test]
    fn rewrites_family_modules() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        let family = "module(\n    name = \"rules_dx_go\",\n    version = \"0.0.0\",\n)\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"x\", version = \"9.9.9\")\n";
        let path = dir.path().join("families/go/MODULE.bazel");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, family).unwrap();
        assert_eq!(bump(dir.path(), "1.2.3", NEW), Ok(9));
        assert_eq!(
            read(dir.path(), "families/go/MODULE.bazel"),
            "module(\n    name = \"rules_dx_go\",\n    version = \"1.2.3\",\n)\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nbazel_dep(name = \"x\", version = \"9.9.9\")\n"
        );
    }

    #[test]
    fn running_twice_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        bump(dir.path(), "1.2.3", NEW).unwrap();
        assert_eq!(bump(dir.path(), "1.2.3", NEW), Ok(0));
    }

    #[test]
    fn refuses_unknown_version_formats() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        for bad in ["1.2", "v1.2.3", "01.2.3", "1.2.x", "1.2.3-", ""] {
            assert!(bump(dir.path(), bad, NEW).is_err(), "{bad}");
        }
        assert!(valid_version("1.2.3-rc.1"));
        assert!(bump(dir.path(), "1.2.3", "abc").is_err());
        assert_eq!(
            read(dir.path(), "cli/prebuilt/pins.bzl"),
            "DX_VERSION = \"0.0.0\"\n"
        );
    }

    #[test]
    fn refuses_a_workflow_without_affected_on_pr() {
        assert!(!has_affected_on_pr(
            "on:\n  workflow_call:\n    inputs:\n      platforms:\n"
        ));
        assert!(has_affected_on_pr(
            "    inputs:\n      affected_on_pr:\n        type: boolean\n"
        ));
    }

    #[test]
    fn names_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let error = bump(dir.path(), "1.2.3", NEW).unwrap_err();
        assert!(error.starts_with("MODULE.bazel"), "{error}");
    }
}
