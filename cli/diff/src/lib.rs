#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchKind {
    Modify,
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePatch<'a> {
    pub path: &'a str,
    pub kind: PatchKind,
    pub original: &'a str,
    pub candidate: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiffError {
    #[error("unrepresentable path {path:?}: paths with tab, carriage return, or line feed cannot be rendered")]
    UnrepresentablePath { path: String },
    #[error("duplicate path {path:?}")]
    DuplicatePath { path: String },
    #[error("create {path:?} carries original bytes")]
    CreateWithOriginal { path: String },
    #[error("cannot create empty {path:?}: a unified diff has no hunk for a file with no content")]
    EmptyCreate { path: String },
    #[error("no change for {path:?}: candidate is identical to original")]
    NoopPatch { path: String },
}

const CONTEXT: usize = 3;

fn check_path(path: &str) -> Result<(), DiffError> {
    if path.contains(['\t', '\r', '\n']) {
        return Err(DiffError::UnrepresentablePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

pub fn render_patch(files: &[FilePatch<'_>]) -> Result<String, DiffError> {
    let mut ordered: Vec<&FilePatch<'_>> = files.iter().collect();
    ordered.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let mut seen: Vec<&str> = Vec::with_capacity(ordered.len());
    for file in &ordered {
        check_path(file.path)?;
        if seen.contains(&file.path) {
            return Err(DiffError::DuplicatePath {
                path: (*file.path).to_owned(),
            });
        }
        seen.push(file.path);
        match file.kind {
            PatchKind::Create if !file.original.is_empty() => {
                return Err(DiffError::CreateWithOriginal {
                    path: file.path.to_owned(),
                });
            }
            PatchKind::Create if file.candidate.is_empty() => {
                return Err(DiffError::EmptyCreate {
                    path: file.path.to_owned(),
                });
            }
            PatchKind::Modify if file.original == file.candidate => {
                return Err(DiffError::NoopPatch {
                    path: file.path.to_owned(),
                });
            }
            PatchKind::Create | PatchKind::Modify => {}
        }
    }
    let mut out = String::new();
    for file in ordered {
        render_file(&mut out, file)?;
    }
    Ok(out)
}

fn render_file(out: &mut String, file: &FilePatch<'_>) -> Result<(), DiffError> {
    match file.kind {
        PatchKind::Modify => {
            out.push_str("--- a/");
            out.push_str(file.path);
            out.push('\n');
        }
        PatchKind::Create => {
            out.push_str("--- /dev/null\n");
        }
    }
    out.push_str("+++ b/");
    out.push_str(file.path);
    out.push('\n');
    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .diff_lines(file.original, file.candidate);
    let mut unified = diff.unified_diff();
    unified.context_radius(CONTEXT);
    for hunk in unified.iter_hunks() {
        let ops = hunk.ops();
        let first = &ops[0];
        let last = &ops[ops.len() - 1];
        let old_start = first.old_range().start;
        let new_start = first.new_range().start;
        let old_len = last.old_range().end - old_start;
        let new_len = last.new_range().end - new_start;
        let old_head = if old_len == 0 {
            old_start
        } else {
            old_start + 1
        };
        let new_head = if new_len == 0 {
            new_start
        } else {
            new_start + 1
        };
        out.push_str(&format!(
            "@@ -{old_head},{old_len} +{new_head},{new_len} @@\n"
        ));
        for change in hunk.iter_changes() {
            let tag = match change.tag() {
                similar::ChangeTag::Equal => ' ',
                similar::ChangeTag::Delete => '-',
                similar::ChangeTag::Insert => '+',
            };
            out.push(tag);
            out.push_str(&change.to_string_lossy());
            if change.missing_newline() {
                out.push('\n');
                out.push_str("\\ No newline at end of file\n");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modify<'a>(path: &'a str, original: &'a str, candidate: &'a str) -> FilePatch<'a> {
        FilePatch {
            path,
            kind: PatchKind::Modify,
            original,
            candidate,
        }
    }

    #[test]
    fn empty_input_renders_no_bytes() {
        assert_eq!(render_patch(&[]).expect("empty"), "");
    }

    #[test]
    fn single_line_change() {
        let got = render_patch(&[modify("a.txt", "one\ntwo\n", "one\nTWO\n")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n"
        );
    }

    #[test]
    fn create_uses_dev_null_header() {
        let patch = FilePatch {
            path: "new/f.txt",
            kind: PatchKind::Create,
            original: "",
            candidate: "hi\n",
        };
        let got = render_patch(&[patch]).expect("patch");
        assert_eq!(
            got,
            "--- /dev/null\n+++ b/new/f.txt\n@@ -0,0 +1,1 @@\n+hi\n"
        );
    }

    #[test]
    fn empty_create_is_rejected() {
        // git and patch both reject a header pair with no hunk, and the only plain hunk that
        // applies writes a byte the file does not have.
        let patch = FilePatch {
            path: "empty/BUILD.bazel",
            kind: PatchKind::Create,
            original: "",
            candidate: "",
        };
        assert_eq!(
            render_patch(&[patch]).expect_err("must fail"),
            DiffError::EmptyCreate {
                path: "empty/BUILD.bazel".to_owned()
            }
        );
    }

    #[test]
    fn canonical_headers_always_carry_counts() {
        let got = render_patch(&[modify("a.txt", "a\n", "b\n")]).expect("patch");
        assert_eq!(got, "--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-a\n+b\n");
    }

    #[test]
    fn removed_final_newline_marks_new_only() {
        let got = render_patch(&[modify("a.txt", "one\n", "one")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-one\n+one\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn files_sort_by_path_bytes() {
        let files = [modify("b.txt", "x\n", "y\n"), modify("a.txt", "x\n", "y\n")];
        let got = render_patch(&files).expect("patch");
        assert!(got.starts_with("--- a/a.txt\n"));
        assert!(got.contains("--- a/b.txt\n"));
    }

    #[test]
    fn unrepresentable_paths_fail() {
        for bad in ["a\tb", "a\rb", "a\nb"] {
            let err = render_patch(&[modify(bad, "x\n", "y\n")]).expect_err("must fail");
            assert_eq!(
                err,
                DiffError::UnrepresentablePath {
                    path: bad.to_owned()
                }
            );
        }
    }

    #[test]
    fn duplicate_paths_fail() {
        let files = [modify("a.txt", "x\n", "y\n"), modify("a.txt", "x\n", "z\n")];
        assert_eq!(
            render_patch(&files).expect_err("must fail"),
            DiffError::DuplicatePath {
                path: "a.txt".to_owned()
            }
        );
    }

    #[test]
    fn create_with_original_fails() {
        let patch = FilePatch {
            path: "n.txt",
            kind: PatchKind::Create,
            original: "x\n",
            candidate: "y\n",
        };
        assert_eq!(
            render_patch(&[patch]).expect_err("must fail"),
            DiffError::CreateWithOriginal {
                path: "n.txt".to_owned()
            }
        );
    }

    #[test]
    fn noop_modify_fails() {
        assert_eq!(
            render_patch(&[modify("a.txt", "x\n", "x\n")]).expect_err("must fail"),
            DiffError::NoopPatch {
                path: "a.txt".to_owned()
            }
        );
    }

    #[test]
    fn missing_final_newline_gets_marker() {
        let got = render_patch(&[modify("a.txt", "one\ntwo", "one\nTWO")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n one\n-two\n\\ No newline at end of file\n+TWO\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn added_final_newline_marks_old_only() {
        let got = render_patch(&[modify("a.txt", "one", "one\n")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-one\n\\ No newline at end of file\n+one\n"
        );
    }

    #[test]
    fn distant_changes_split_hunks() {
        let old: String = (0..20).map(|i| format!("line {i}\n")).collect();
        let mut new = old.clone();
        new = new.replacen("line 1\n", "LINE 1\n", 1);
        new = new.replacen("line 18\n", "LINE 18\n", 1);
        let got = render_patch(&[modify("a.txt", &old, &new)]).expect("patch");
        assert_eq!(got.matches("@@ ").count(), 2);
        assert!(got.contains("-line 1\n+LINE 1\n"));
        assert!(got.contains("-line 18\n+LINE 18\n"));
    }

    #[test]
    fn multibyte_lines_compare_by_bytes() {
        let got = render_patch(&[modify("a.txt", "héllo\n", "héllo!\n")]).expect("patch");
        assert!(got.contains("-héllo\n+héllo!\n"));
    }

    #[test]
    fn insertion_at_start_uses_zero_head() {
        let got = render_patch(&[modify("a.txt", "b\n", "a\nb\n")]).expect("patch");
        assert_eq!(got, "--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,2 @@\n+a\n b\n");
    }

    #[test]
    fn deletion_renders_minus_only() {
        let got = render_patch(&[modify("a.txt", "a\nb\nc\n", "a\nc\n")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,2 @@\n a\n-b\n c\n"
        );
    }

    #[test]
    fn full_deletion_renders_zero_new_head() {
        let got = render_patch(&[modify("a.txt", "a\nb\n", "")]).expect("patch");
        assert_eq!(got, "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +0,0 @@\n-a\n-b\n");
    }

    #[test]
    fn context_line_at_eof_gets_marker() {
        let got = render_patch(&[modify("a.txt", "a\nb\nc\nd", "a\nB\nc\nd")]).expect("patch");
        assert_eq!(
            got,
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,4 +1,4 @@\n a\n-b\n+B\n c\n d\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn display_is_human_readable() {
        let unrepresentable = DiffError::UnrepresentablePath {
            path: "a\tb".to_owned(),
        };
        assert_eq!(
            unrepresentable.to_string(),
            "unrepresentable path \"a\\tb\": paths with tab, carriage return, or line feed cannot be rendered"
        );
        assert!(!unrepresentable
            .to_string()
            .contains("UnrepresentablePath {"));
        assert_eq!(
            DiffError::DuplicatePath {
                path: "a".to_owned()
            }
            .to_string(),
            "duplicate path \"a\""
        );
        assert_eq!(
            DiffError::CreateWithOriginal {
                path: "n".to_owned()
            }
            .to_string(),
            "create \"n\" carries original bytes"
        );
        assert_eq!(
            DiffError::EmptyCreate {
                path: "e".to_owned()
            }
            .to_string(),
            "cannot create empty \"e\": a unified diff has no hunk for a file with no content"
        );
        assert_eq!(
            DiffError::NoopPatch {
                path: "a".to_owned()
            }
            .to_string(),
            "no change for \"a\": candidate is identical to original"
        );
    }
}
