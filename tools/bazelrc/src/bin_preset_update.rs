#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::Write;
use std::path::PathBuf;

fn run(
    argv: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    resolve: impl FnOnce() -> std::io::Result<PathBuf>,
) -> i32 {
    let verify_only = argv.len() == 2 && argv[1] == "--verify-only";
    if argv.len() != 1 && !verify_only {
        return dx_preset::bin_usage(err);
    }
    let workspace = match resolve() {
        Ok(workspace) => workspace,
        Err(error) => {
            return dx_preset::bin_cannot(
                err,
                format!("preset.update: cannot resolve workspace: {error}"),
            );
        }
    };
    let source: PathBuf = dx_preset::source_dir(&workspace);
    if !source.is_dir() {
        return dx_preset::bin_cannot(
            err,
            format!(
                "preset.update: source directory '{}' not found",
                source.display()
            ),
        );
    }
    if verify_only {
        match dx_preset::check_preset(&workspace) {
            Ok(()) => {
                let _ = writeln!(out, "preset.update: preset.bazelrc verified");
                0
            }
            Err(dx_preset::PresetError::Stale { detail }) => {
                let _ = writeln!(out, "{detail}");
                1
            }
            Err(error) => dx_preset::bin_cannot(err, error),
        }
    } else {
        match dx_preset::update_preset(&workspace) {
            Ok(()) => {
                let _ = writeln!(out, "preset.update: wrote preset.bazelrc");
                0
            }
            Err(error) => dx_preset::bin_cannot(err, error),
        }
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    let code = run(&argv, &mut out, &mut err, dx_preset::resolve_workspace);
    std::process::exit(code);
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_BAZELRC: &str =
        "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n";

    fn captured(
        args: &[&str],
        resolve: impl FnOnce() -> std::io::Result<PathBuf>,
    ) -> (i32, String, String) {
        let argv: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let code = run(&argv, &mut out, &mut err, resolve);
        (
            code,
            String::from_utf8(out).expect("stdout is utf8"),
            String::from_utf8(err).expect("stderr is utf8"),
        )
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch")
    }

    fn with_source(root: &std::path::Path) {
        std::fs::create_dir_all(dx_preset::source_dir(root)).expect("source dir");
    }

    fn fragment(root: &std::path::Path) -> PathBuf {
        dx_preset::preset_paths(root).1
    }

    #[test]
    fn usage_rejects_extra_args_before_the_workspace_is_resolved() {
        for args in [
            vec!["preset_update", "--other"],
            vec!["preset_update", "--verify-only", "extra"],
            vec!["preset_update", ""],
        ] {
            let (code, out, err) = captured(&args, || {
                panic!("a usage error must not resolve a workspace: {args:?}")
            });
            assert_eq!(code, 1, "{args:?}");
            assert_eq!(out, "", "{args:?}");
            assert!(
                err.contains("usage: preset.update [--verify-only]"),
                "{err}"
            );
        }
    }

    #[test]
    fn an_unresolvable_workspace_is_reported_on_stderr() {
        let (code, out, err) = captured(&["preset_update"], || {
            Err(std::io::Error::other("no workspace here"))
        });
        assert_eq!(code, 1);
        assert_eq!(out, "");
        assert!(
            err.contains("preset.update: cannot resolve workspace: no workspace here"),
            "{err}"
        );
    }

    #[test]
    fn a_missing_source_dir_is_named_before_any_preset_is_read() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        let (code, out, err) = captured(&["preset_update"], || Ok(root.clone()));
        assert_eq!(code, 1);
        assert_eq!(out, "");
        assert!(
            err.contains(&format!(
                "source directory '{}' not found",
                dx_preset::source_dir(&root).display()
            )),
            "{err}"
        );
        assert!(!fragment(&root).exists(), "a failed run must write nothing");
    }

    #[test]
    fn verify_reports_a_fresh_preset_on_stdout() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        with_source(&root);
        std::fs::write(root.join(".bazelrc"), ROOT_BAZELRC).expect("root bazelrc");
        std::fs::write(fragment(&root), dx_preset::render_fragment()).expect("fragment");
        let (code, out, err) = captured(&["preset_update", "--verify-only"], || Ok(root));
        assert_eq!(code, 0);
        assert!(
            out.contains("preset.update: preset.bazelrc verified"),
            "{out}"
        );
        assert_eq!(err, "");
    }

    #[test]
    fn verify_reports_a_stale_preset_on_stdout_and_leaves_it_alone() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        with_source(&root);
        std::fs::write(root.join(".bazelrc"), ROOT_BAZELRC).expect("root bazelrc");
        std::fs::write(fragment(&root), "# dirty\n").expect("dirty fragment");
        let (code, out, err) = captured(&["preset_update", "--verify-only"], || Ok(root));
        assert_eq!(code, 1);
        assert!(out.contains("is stale"), "{out}");
        assert_eq!(err, "", "a stale preset is a report, not a failure: {err}");
        assert_eq!(
            std::fs::read_to_string(fragment(dir.path())).expect("fragment"),
            "# dirty\n",
            "verify must not rewrite the preset"
        );
    }

    #[test]
    fn verify_sends_a_colliding_root_bazelrc_to_stderr() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        with_source(&root);
        std::fs::write(fragment(&root), dx_preset::render_fragment()).expect("fragment");
        std::fs::write(
            root.join(".bazelrc"),
            format!("{ROOT_BAZELRC}common --enable_bzlmod\n"),
        )
        .expect("collision");
        let (code, out, err) = captured(&["preset_update", "--verify-only"], || Ok(root));
        assert_eq!(code, 1);
        assert_eq!(out, "");
        assert!(err.contains("duplicates preset lines"), "{err}");
    }

    #[test]
    fn update_writes_the_rendered_preset() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        with_source(&root);
        std::fs::write(root.join(".bazelrc"), ROOT_BAZELRC).expect("root bazelrc");
        let (code, out, err) = captured(&["preset_update"], || Ok(root));
        assert_eq!(code, 0);
        assert!(out.contains("preset.update: wrote preset.bazelrc"), "{out}");
        assert_eq!(err, "");
        assert_eq!(
            std::fs::read_to_string(fragment(dir.path())).expect("fragment"),
            dx_preset::render_fragment()
        );
    }

    #[test]
    fn update_refuses_to_write_when_the_root_bazelrc_duplicates_a_preset_line() {
        let dir = scratch();
        let root = dir.path().to_path_buf();
        with_source(&root);
        std::fs::write(
            root.join(".bazelrc"),
            format!("{ROOT_BAZELRC}common --enable_bzlmod\n"),
        )
        .expect("collision");
        let (code, out, err) = captured(&["preset_update"], || Ok(root.clone()));
        assert_eq!(code, 1);
        assert_eq!(out, "");
        assert!(err.contains("duplicates preset lines"), "{err}");
        assert!(
            !fragment(&root).exists(),
            "a rejected run must not write the preset"
        );
    }
}
