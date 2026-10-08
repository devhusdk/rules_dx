use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, summaries_suppressed};
use dx_process::pre_exec_code;

pub(crate) const CODE_INIT_FAILED: &str = "init_failed";

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
    if let Err(error) = dx_adopt::validate_init_module(module) {
        let _ = writeln!(err, "dx: {error}");
        return pre_exec_code();
    }
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
                    return operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string());
                }
            }
        }
        return 0;
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
        Err(error) => operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string()),
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
        let inv = invocation(&["init"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        assert!(root.join(".devcontainer/devcontainer.json").exists());
    }

    #[test]
    fn init_rejects_hostile_module_pre_exec_without_writing() {
        let inv = invocation(&["init", "../escape"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-module-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid module"), "{err}");
        let entries: Vec<_> = std::fs::read_dir(&root)
            .expect("list")
            .collect::<Result<_, _>>()
            .expect("entries");
        assert!(entries.is_empty());
    }

    #[test]
    fn init_accepts_repository_module() {
        let inv = invocation(&["init", "github.com/org/repo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-repo-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read");
        assert!(ci.contains("github.com/org/repo/.github/workflows/reusable-consumer.yml"));
    }
}
