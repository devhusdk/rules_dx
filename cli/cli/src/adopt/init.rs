use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, summaries_suppressed};

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
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            for file in dx_adopt::plan_init_files(module) {
                if let Err(exit) = check_stdout_write(writeln!(out, "would write {}", file.path)) {
                    return exit;
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
    use crate::adopt::execute_adoption;
    use crate::adopt::test_support::{env, invocation};

    #[test]
    fn init_dry_run_lists_without_writing() {
        let inv = invocation(&["init", "--dry-run", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-dry-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out).expect("out").contains(".dx/version"));
        assert!(!root.join(".dx/version").exists());
    }

    #[test]
    fn init_applies_absent_only() {
        let inv = invocation(&["init"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-apply-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        assert!(root.join(".devcontainer/devcontainer.json").exists());
    }
}
