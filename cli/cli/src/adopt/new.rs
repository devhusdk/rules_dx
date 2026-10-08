use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, summaries_suppressed};
use dx_process::pre_exec_code;

pub(crate) const CODE_NEW_FAILED: &str = "new_failed";

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
    if let Err(error) = dx_adopt::validate_new_destination(name) {
        let _ = writeln!(err, "dx: {error}");
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
                    return operational(invocation, out, err, CODE_NEW_FAILED, &error.to_string());
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
        Err(error) => operational(invocation, out, err, CODE_NEW_FAILED, &error.to_string()),
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
    fn new_rejects_traversal_name_pre_exec_without_writing() {
        let inv = invocation(&["new", "rust", "../escape"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-traversal-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid destination"), "{err}");
        let entries: Vec<_> = std::fs::read_dir(&root)
            .expect("list")
            .collect::<Result<_, _>>()
            .expect("entries");
        assert!(entries.is_empty());
    }

    #[test]
    fn new_rejects_reserved_name_pre_exec() {
        let inv = invocation(&["new", "go", "con"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-reserved-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("reserved Windows name"), "{err}");
    }

    #[test]
    fn new_derives_package_for_space_destination() {
        let inv = invocation(&["new", "go", "my app"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-space-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(root.join("my app/go.mod")).expect("read"),
            "module my_app\n\ngo 1.26\n"
        );
    }
}
