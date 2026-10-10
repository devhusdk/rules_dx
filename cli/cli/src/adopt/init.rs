use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, scaffold_check, summaries_suppressed};
use dx_process::pre_exec_code;

pub(crate) const CODE_INIT_FAILED: &str = "init_failed";

fn invalid_module_code(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_adopt::AdoptError,
) -> i32 {
    match error {
        dx_adopt::AdoptError::InitInvalidModule { .. } => {
            let _ = writeln!(err, "dx: {error}");
            pre_exec_code()
        }
        _ => operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string()),
    }
}

pub(crate) fn execute_init(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let module = invocation
        .targets
        .first()
        .map_or("my_project", String::as_str);
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            match dx_adopt::plan_init_files(module) {
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
                    return invalid_module_code(invocation, out, err, &error);
                }
            }
        }
        return 0;
    }
    if !invocation.applies() {
        return match dx_adopt::plan_init_files(module) {
            Ok(files) => {
                let rerun = if invocation.targets.is_empty() {
                    "dx init --apply".to_owned()
                } else {
                    format!("dx init --apply {module}")
                };
                scaffold_check(invocation, workspace, out, err, "init", &rerun, &files)
            }
            Err(error) => invalid_module_code(invocation, out, err, &error),
        };
    }
    match dx_adopt::apply_init(workspace, module) {
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
        Err(error) => invalid_module_code(invocation, out, err, &error),
    }
}

#[cfg(test)]
mod tests {
    use crate::adopt::test_support::{invocation, run};
    use dx_process::pre_exec_code;

    #[test]
    fn init_dry_run_lists_without_writing() {
        let inv = invocation(&["init", "--dry-run", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains(".dx/version"));
        assert!(!root.join(".dx/version").exists());
    }

    #[test]
    fn init_applies_absent_only() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        assert!(root.join(".devcontainer/devcontainer.json").exists());
    }

    #[test]
    fn init_default_checks_without_writing() {
        for words in [vec!["init"], vec!["init", "--check"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-init-check-");
            let root = scratch.path().to_path_buf();
            let (code, _out, err) = run(&inv, &root);
            assert_eq!(code, 1, "{words:?}");
            assert!(err.contains("init_failed"), "{words:?} {err}");
            assert!(err.contains("would write 9 file(s)"), "{words:?} {err}");
            assert!(err.contains("dx init --apply"), "{words:?} {err}");
            assert!(!root.join(".dx/version").exists(), "{words:?}");
            assert!(!root.join(".devcontainer").exists(), "{words:?}");
        }
    }

    #[test]
    fn init_check_passes_once_applied() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-current-");
        let root = scratch.path().to_path_buf();
        let (code, _, _) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        for words in [vec!["init"], vec!["init", "--check"]] {
            let (code, out, err) = run(&invocation(&words), &root);
            assert_eq!(code, 0, "{words:?} {err}");
            assert!(out.contains("init current"), "{words:?} {out}");
            assert!(err.is_empty(), "{words:?} {err}");
        }
    }

    #[test]
    fn init_rejects_invalid_modules_pre_exec_without_writing() {
        for module in ["Bad Name", "../evil", "with\nnewline", "semi;colon"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            let inv = invocation(&["init", module]);
            let (code, _out, err) = run(&inv, &root);
            assert_eq!(code, pre_exec_code(), "{module:?}");
            assert!(err.contains("invalid module name"), "{err}");
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
            assert_eq!(entries, vec!["sentinel".to_owned()], "{module:?}");
        }
    }
}
