#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::{DEFAULT_TIMEOUT, RunError, run_checked};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: bazel run //deploy/private/release:bump_version -- <version> [--workflow-ref <commit>]\n\nSets the rules_dx version, the starter workflow pin and the assert-no-secret and remote-cache-helper action pins in every file that carries them and refreshes tools/ci/starter_pin.sha256 from the workflows in the checkout.\n--workflow-ref defaults to origin/main, must have the affected_on_pr input and must carry the assert-no-secret and remote-cache-helper actions.";
const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";
const DOCS_WORKFLOW: &str = ".github/workflows/reusable-docs.yml";
const SCAN_ACTION: &str = ".github/actions/assert-no-secret";
const SCAN_ACTION_PIN: &str = "devhusdk/rules_dx/.github/actions/assert-no-secret@";
const HELPER_ACTION: &str = ".github/actions/remote-cache-helper";
const HELPER_ACTION_PIN: &str = "devhusdk/rules_dx/.github/actions/remote-cache-helper@";
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
            "cli/shim/src/protocol.rs",
            vec![version("SHIM_VERSION: &str = \"")],
        ),
        (
            "cli/adopt/src/scaffold.rs",
            vec![pin("CONSUMER_WORKFLOW_PIN: &str = \"")],
        ),
        (
            "examples/docs-ci/caller.yml",
            vec![pin("reusable-docs.yml@")],
        ),
        (
            CONSUMER_WORKFLOW,
            vec![pin(SCAN_ACTION_PIN), pin(HELPER_ACTION_PIN)],
        ),
        (
            DOCS_WORKFLOW,
            vec![pin(SCAN_ACTION_PIN), pin(HELPER_ACTION_PIN)],
        ),
    ]
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

#[derive(Debug)]
enum BumpError {
    Version {
        version: String,
    },
    Commit {
        commit: String,
    },
    NoMarker {
        file: &'static str,
        marker: &'static str,
    },
    DigestLine {
        line: String,
    },
    DigestInput(PathIoError),
    Io(PathIoError),
    Git {
        args: String,
        source: RunError,
    },
    MissingInput {
        commit: String,
        file: String,
        source: RunError,
    },
    NoAffectedOnPr {
        commit: String,
    },
    WorkflowRefValue,
    Unexpected {
        arg: String,
    },
    Usage,
    Workspace,
}

impl fmt::Display for BumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version { version } => write!(f, "`{version}` is not a version like 1.2.3"),
            Self::Commit { commit } => write!(f, "`{commit}` is not a 40 character commit"),
            Self::NoMarker { file, marker } => write!(f, "{file}: no line with `{marker}`"),
            Self::DigestLine { line } => write!(f, "{DIGEST_RECORD}: malformed line `{line}`"),
            Self::DigestInput(source) => write!(f, "{DIGEST_RECORD}: {source}"),
            Self::Io(source) => write!(f, "{source}"),
            Self::Git { args, source } => write!(f, "git {args}: {source}"),
            Self::MissingInput { commit, file, .. } => write!(f, "{commit} has no {file}"),
            Self::NoAffectedOnPr { commit } => write!(
                f,
                "{commit} has no affected_on_pr input in {CONSUMER_WORKFLOW}"
            ),
            Self::WorkflowRefValue => f.write_str("--workflow-ref needs a commit"),
            Self::Unexpected { arg } => write!(f, "unexpected argument `{arg}`\n{USAGE}"),
            Self::Usage => f.write_str(USAGE),
            Self::Workspace => f.write_str(
                "BUILD_WORKSPACE_DIRECTORY must be an absolute path (run with bazel run)",
            ),
        }
    }
}

impl std::error::Error for BumpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DigestInput(source) | Self::Io(source) => Some(source),
            Self::Git { source, .. } | Self::MissingInput { source, .. } => Some(source),
            Self::Version { .. }
            | Self::Commit { .. }
            | Self::NoMarker { .. }
            | Self::DigestLine { .. }
            | Self::NoAffectedOnPr { .. }
            | Self::WorkflowRefValue
            | Self::Unexpected { .. }
            | Self::Usage
            | Self::Workspace => None,
        }
    }
}

fn rewrite(text: &str, edit: &Edit, value: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut previous = "";
    let mut hits = 0;
    for line in text.split_inclusive('\n') {
        let allowed = edit.after.is_none_or(|after| previous.contains(after));
        match line.find(edit.marker).filter(|_| allowed) {
            Some(at) => {
                let start = at + edit.marker.len();
                let (head, tail) = line.split_at_checked(start).unwrap_or((line, ""));
                let rest = tail
                    .find(|c: char| matches!(c, '"' | '/') || c.is_whitespace())
                    .and_then(|n| tail.get(n..))
                    .unwrap_or("");
                out.push_str(head);
                out.push_str(value);
                out.push_str(rest);
                hits += 1;
            }
            None => out.push_str(line),
        }
        previous = line;
    }
    (hits > 0).then_some(out)
}

fn redigest(root: &Path, record: &str) -> Result<String, BumpError> {
    let mut out = String::with_capacity(record.len());
    for line in record.lines() {
        let (_, path) = line.split_once("  ").ok_or_else(|| BumpError::DigestLine {
            line: line.to_owned(),
        })?;
        let bytes = path_io::read(&root.join(path)).map_err(BumpError::DigestInput)?;
        writeln!(out, "{}  {path}", dx_digest::sha256_hex(&bytes)).ok();
    }
    Ok(out)
}

fn write_checkout(path: &Path, text: &str) -> Result<(), BumpError> {
    dx_atomic_fs::write_atomic(path, text.as_bytes()).map_err(|source| {
        BumpError::Io(PathIoError::Write {
            path: path.to_path_buf(),
            source,
        })
    })
}

fn bump(root: &Path, version: &str, commit: &str) -> Result<usize, BumpError> {
    if !valid_version(version) {
        return Err(BumpError::Version {
            version: version.to_owned(),
        });
    }
    if !valid_commit(commit) {
        return Err(BumpError::Commit {
            commit: commit.to_owned(),
        });
    }
    let mut changed = Vec::new();
    for (file, edits) in targets() {
        let path = root.join(file);
        let old = path_io::read_to_string(&path).map_err(BumpError::Io)?;
        let mut text = old.clone();
        for edit in &edits {
            let value = if edit.pin { commit } else { version };
            text = rewrite(&text, edit, value).ok_or(BumpError::NoMarker {
                file,
                marker: edit.marker,
            })?;
        }
        if text != old {
            changed.push((path, text));
        }
    }
    let record = root.join(DIGEST_RECORD);
    let old = path_io::read_to_string(&record).map_err(BumpError::Io)?;
    for (path, text) in &changed {
        write_checkout(path, text)?;
    }
    let mut count = changed.len();
    let text = redigest(root, &old)?;
    if text != old {
        write_checkout(&record, &text)?;
        count += 1;
    }
    Ok(count)
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, RunError> {
    let out = run_checked(dx_process::capture::git(root).args(args), DEFAULT_TIMEOUT)?;
    Ok(out.stdout_text().trim().to_owned())
}

fn git(root: &Path, args: &[&str]) -> Result<String, BumpError> {
    git_output(root, args).map_err(|source| BumpError::Git {
        args: args.join(" "),
        source,
    })
}

fn has_affected_on_pr(workflow: &str) -> bool {
    workflow
        .lines()
        .any(|line| line.trim_start().starts_with("affected_on_pr:"))
}

fn resolve_commit(root: &Path, reference: &str) -> Result<String, BumpError> {
    let commit = git(root, &["rev-parse", &format!("{reference}^{{commit}}")])?;
    let workflow = git(root, &["show", &format!("{commit}:{CONSUMER_WORKFLOW}")])?;
    if !has_affected_on_pr(&workflow) {
        return Err(BumpError::NoAffectedOnPr { commit });
    }
    for file in [
        format!("{SCAN_ACTION}/action.yml"),
        format!("{HELPER_ACTION}/helper.cjs"),
    ] {
        if let Err(source) = git_output(root, &["cat-file", "-e", &format!("{commit}:{file}")]) {
            return Err(BumpError::MissingInput {
                commit,
                file,
                source,
            });
        }
    }
    Ok(commit)
}

fn run(args: &[String], workspace: Option<PathBuf>) -> Result<String, BumpError> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(USAGE.to_owned());
    }
    let mut version = None;
    let mut reference = "origin/main".to_owned();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workflow-ref" => {
                iter.next()
                    .ok_or(BumpError::WorkflowRefValue)?
                    .clone_into(&mut reference);
            }
            other if !other.starts_with('-') && version.is_none() => version = Some(other),
            other => {
                return Err(BumpError::Unexpected {
                    arg: other.to_owned(),
                });
            }
        }
    }
    let version = version.ok_or(BumpError::Usage)?;
    let root = workspace
        .filter(|path| path.is_absolute())
        .ok_or(BumpError::Workspace)?;
    if !valid_version(version) {
        return Err(BumpError::Version {
            version: version.to_owned(),
        });
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
    use std::fs;

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
                "cli/shim/src/protocol.rs",
                "pub const SHIM_VERSION: &str = \"0.0.0\";\n",
            ),
            (
                "cli/adopt/src/scaffold.rs",
                "pub const CONSUMER_WORKFLOW_PIN: &str = \"e97ab7b550d08e047c35edc5f50c7d242227842f\";\n",
            ),
            (
                ".github/workflows/reusable-docs.yml",
                "name: docs\n        uses: devhusdk/rules_dx/.github/actions/assert-no-secret@e97ab7b550d08e047c35edc5f50c7d242227842f # main\n        uses: devhusdk/rules_dx/.github/actions/remote-cache-helper@e97ab7b550d08e047c35edc5f50c7d242227842f # main\n",
            ),
            (
                ".github/workflows/reusable-consumer.yml",
                "name: consumer\n        uses: devhusdk/rules_dx/.github/actions/remote-cache-helper@e97ab7b550d08e047c35edc5f50c7d242227842f # main\n        uses: devhusdk/rules_dx/.github/actions/assert-no-secret@e97ab7b550d08e047c35edc5f50c7d242227842f # main\n        uses: devhusdk/rules_dx/.github/actions/assert-no-secret@e97ab7b550d08e047c35edc5f50c7d242227842f # main\n",
            ),
            (
                "tools/ci/starter_pin.sha256",
                "0000000000000000000000000000000000000000000000000000000000000000  .github/workflows/reusable-docs.yml\n",
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
        assert_eq!(bump(dir.path(), "1.2.3", NEW).unwrap(), 9);
        assert!(read(dir.path(), "MODULE.bazel").contains("    version = \"1.2.3\","));
        assert!(read(dir.path(), "MODULE.bazel").contains("name = \"x\", version = \"9.9.9\""));
        assert_eq!(
            read(dir.path(), "cli/prebuilt/pins.bzl"),
            "DX_VERSION = \"1.2.3\"\n"
        );
        let version = read(dir.path(), "cli/adopt/src/version.rs");
        assert!(version.contains("DX_VERSION: &str = \"1.2.3\";"));
        assert!(version.contains("MODULE_VERSION: &str = \"1.2.3\";"));
        assert_eq!(
            read(dir.path(), "cli/shim/src/protocol.rs"),
            "pub const SHIM_VERSION: &str = \"1.2.3\";\n"
        );
        assert!(read(dir.path(), "cli/adopt/src/scaffold.rs").contains(&format!("\"{NEW}\";")));
        let docs = read(dir.path(), ".github/workflows/reusable-docs.yml");
        assert_eq!(
            docs,
            format!(
                "name: docs\n        uses: {SCAN_ACTION_PIN}{NEW} # main\n        uses: {HELPER_ACTION_PIN}{NEW} # main\n"
            )
        );
        let consumer = read(dir.path(), ".github/workflows/reusable-consumer.yml");
        assert_eq!(
            consumer
                .matches(&format!("{SCAN_ACTION_PIN}{NEW} # main\n"))
                .count(),
            2
        );
        assert_eq!(
            consumer
                .matches(&format!("{HELPER_ACTION_PIN}{NEW} # main\n"))
                .count(),
            1
        );
        assert!(!consumer.contains(OLD));
        assert_eq!(
            read(dir.path(), "tools/ci/starter_pin.sha256"),
            format!(
                "{}  .github/workflows/reusable-docs.yml\n",
                dx_digest::sha256_hex(docs.as_bytes())
            )
        );
        let docs_ci = read(dir.path(), "examples/docs-ci/caller.yml");
        assert!(docs_ci.contains(&format!("reusable-docs.yml@{NEW}\n")));
        assert!(docs_ci.contains("docs_scope: \"//...\""));
        assert!(!docs_ci.contains(OLD));
    }

    #[test]
    fn running_twice_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        bump(dir.path(), "1.2.3", NEW).unwrap();
        assert_eq!(bump(dir.path(), "1.2.3", NEW).unwrap(), 0);
    }

    #[test]
    fn refuses_unknown_version_formats() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        for bad in ["1.2", "v1.2.3", "01.2.3", "1.2.x", "1.2.3-", ""] {
            let error = bump(dir.path(), bad, NEW).unwrap_err();
            assert!(matches!(error, BumpError::Version { .. }), "{bad}");
            assert_eq!(
                error.to_string(),
                format!("`{bad}` is not a version like 1.2.3")
            );
        }
        assert!(valid_version("1.2.3-rc.1"));
        let error = bump(dir.path(), "1.2.3", "abc").unwrap_err();
        assert!(matches!(error, BumpError::Commit { .. }));
        assert_eq!(error.to_string(), "`abc` is not a 40 character commit");
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
        assert!(
            matches!(error, BumpError::Io(PathIoError::Read { .. })),
            "{error}"
        );
        assert!(error.to_string().contains("MODULE.bazel"), "{error}");
    }

    #[test]
    fn names_a_file_without_its_marker() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        fs::write(dir.path().join("cli/prebuilt/pins.bzl"), "x\n").unwrap();
        let error = bump(dir.path(), "1.2.3", NEW).unwrap_err();
        assert!(matches!(error, BumpError::NoMarker { .. }), "{error}");
        assert_eq!(
            error.to_string(),
            "cli/prebuilt/pins.bzl: no line with `DX_VERSION = \"`"
        );
    }

    #[test]
    fn names_a_malformed_digest_line() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        fs::write(dir.path().join(DIGEST_RECORD), "nospace\n").unwrap();
        let error = bump(dir.path(), "1.2.3", NEW).unwrap_err();
        assert!(matches!(error, BumpError::DigestLine { .. }), "{error}");
        assert_eq!(
            error.to_string(),
            "tools/ci/starter_pin.sha256: malformed line `nospace`"
        );
    }

    #[test]
    fn rejects_bad_arguments_before_running_git() {
        let args = |list: &[&str]| list.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        let root = Some(PathBuf::from("/nonexistent-checkout"));
        let error = run(&args(&["1.2.3", "--workflow-ref"]), root.clone()).unwrap_err();
        assert!(matches!(error, BumpError::WorkflowRefValue));
        assert_eq!(error.to_string(), "--workflow-ref needs a commit");
        let error = run(&args(&["--bogus"]), root.clone()).unwrap_err();
        assert!(matches!(error, BumpError::Unexpected { .. }));
        assert_eq!(
            error.to_string(),
            format!("unexpected argument `--bogus`\n{USAGE}")
        );
        let error = run(&args(&[]), root.clone()).unwrap_err();
        assert!(matches!(error, BumpError::Usage));
        assert_eq!(error.to_string(), USAGE);
        let error = run(&args(&["1.2.3"]), Some(PathBuf::from("relative"))).unwrap_err();
        assert!(matches!(error, BumpError::Workspace));
        assert_eq!(
            error.to_string(),
            "BUILD_WORKSPACE_DIRECTORY must be an absolute path (run with bazel run)"
        );
        let error = run(&args(&["1.2"]), root).unwrap_err();
        assert!(matches!(error, BumpError::Version { .. }));
        assert_eq!(error.to_string(), "`1.2` is not a version like 1.2.3");
    }
}
