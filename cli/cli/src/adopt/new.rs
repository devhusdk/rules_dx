use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, summaries_suppressed};
use dx_process::pre_exec_code;

pub(crate) const CODE_NEW_FAILED: &str = "new_failed";

fn invalid_name_code(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_adopt::AdoptError,
) -> i32 {
    match error {
        dx_adopt::AdoptError::NewInvalidDestination { .. }
        | dx_adopt::AdoptError::NewInvalidIdentity { .. } => {
            let _ = writeln!(err, "dx: {error}");
            pre_exec_code()
        }
        _ => operational(invocation, out, err, CODE_NEW_FAILED, &error.to_string()),
    }
}

pub(crate) fn execute_new(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let language = invocation.targets.first().map(String::as_str).unwrap_or("");
    let name = invocation
        .targets
        .get(1)
        .map(String::as_str)
        .unwrap_or(dx_adopt::default_new_name());
    if !dx_adopt::new_is_known_language(language) {
        let _ = writeln!(
            err,
            "dx: unknown language for dx new: {language} (want one of {})",
            dx_adopt::new_language_name_list()
        );
        return pre_exec_code();
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            match dx_adopt::plan_new_files(language, name) {
                Ok(files) => {
                    for file in files {
                        if let Err(exit) =
                            check_stdout_write(writeln!(out, "would write {}", file.path))
                        {
                            return exit;
                        }
                    }
                }
                Err(error) => {
                    return invalid_name_code(invocation, out, err, &error);
                }
            }
        }
        return 0;
    }
    match dx_adopt::apply_new(workspace, language, name) {
        Ok(entries) => {
            for entry in entries {
                if entry == "---" {
                    continue;
                }
                if let Some(path) = entry.strip_prefix("refused:") {
                    let _ = writeln!(err, "dx: {path} (absent-only, left untouched)");
                } else if !summaries_suppressed(invocation) {
                    if let Err(exit) = check_stdout_write(writeln!(out, "wrote {entry}")) {
                        return exit;
                    }
                }
            }
            0
        }
        Err(error) => invalid_name_code(invocation, out, err, &error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{invocation, run};

    #[test]
    fn new_dry_run_lists_without_writing() {
        let inv = invocation(&["new", "rust", "demo", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        assert!(text.contains("demo/Cargo.toml"), "{text}");
        assert!(text.contains("demo/.dx/version"), "{text}");
        assert!(!root.join("demo/Cargo.toml").exists());
    }

    #[test]
    fn new_applies_absent_only() {
        let inv = invocation(&["new", "go", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join("demo/go.mod").exists());
        assert!(root.join("demo/.dx/version").exists());
    }

    #[test]
    fn new_rejects_unknown_language_pre_exec() {
        let inv = invocation(&["new", "ruby", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-unknown-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("unknown language"));
    }

    #[test]
    fn new_rejects_escape_names_pre_exec_without_writing() {
        for name in ["../evil", "/tmp/absolute", "with\nnewline", "a\\b"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-escape-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            let inv = invocation(&["new", "rust", name]);
            let (code, _out, err) = run(&inv, &root);
            assert_eq!(code, pre_exec_code(), "{name:?}");
            assert!(err.contains("invalid destination"), "{err}");
            let mut entries: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&root).expect("read") {
                entries.push(
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            assert_eq!(entries, vec!["sentinel".to_owned()], "{name:?}");
        }
    }

    #[test]
    fn new_rejects_unfoldable_identities_pre_exec_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-identity-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "+"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid package identity"), "{err}");
        assert!(!root.join("+").exists());
    }

    #[test]
    fn new_dry_run_rejects_escape_names_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-dry-escape-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "../evil", "--dry-run"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid destination"), "{err}");
    }

    #[test]
    fn new_scaffolds_nested_destinations_with_folded_identities() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-nested-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "teams/My App"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let cargo = std::fs::read_to_string(root.join("teams/My App/Cargo.toml")).expect("cargo");
        assert!(cargo.contains("name = \"my-app\""), "{cargo}");
    }
}
