use std::io;
use std::path::{Path, PathBuf};

use super::rule_graph::is_graph_query;
use super::{QueryResult, QueryRunner};

/// Bump when the cache file layout or the key inputs change.
const FORMAT: &str = "dx-rule-graph-v1";

/// Query runner that keeps the last successful workspace rule-graph query on disk, so a later
/// dx command whose BUILD inputs have not changed skips Bazel entirely.
///
/// The key hashes the query argv and every file the unconfigured graph can depend on: BUILD
/// files, `.bzl` and `.scl` files, module files, the lockfile and the rc files. Anything that
/// changes the graph changes one of them, so a stale hit cannot happen; a changed file only
/// costs one query. The cache is best effort: when the workspace cannot be hashed or the file
/// cannot be read or written, the query simply runs.
pub struct PersistentGraphQuery<'a> {
    inner: &'a dyn QueryRunner,
}

impl<'a> PersistentGraphQuery<'a> {
    pub fn new(inner: &'a dyn QueryRunner) -> Self {
        PersistentGraphQuery { inner }
    }
}

fn cache_path(workspace: &Path) -> PathBuf {
    dx_path::state::local_dir(workspace).join(dx_path::state::RULE_GRAPH)
}

/// Whether a file named `name` can change what `kind('rule', //...)` reports.
fn is_graph_input(name: &str) -> bool {
    matches!(
        name,
        "BUILD"
            | "BUILD.bazel"
            | "MODULE.bazel"
            | "MODULE.bazel.lock"
            | "REPO.bazel"
            | "WORKSPACE"
            | "WORKSPACE.bazel"
            | "WORKSPACE.bzlmod"
            | ".bazelignore"
            | ".bazelversion"
    ) || name.ends_with(".bzl")
        || name.ends_with(".scl")
        || name.ends_with(".MODULE.bazel")
        || name.ends_with("bazelrc")
}

/// The cache key for `argv` in `workspace`, or `None` when the workspace cannot be walked or
/// one of its graph inputs cannot be read.
fn cache_key(argv: &[String], workspace: &Path) -> Option<String> {
    let mut inputs: Vec<String> = Vec::new();
    dx_adopt::load_graph::walk_workspace(workspace, &[".git"], |name, rel, _| {
        if is_graph_input(name) {
            inputs.push(rel.to_owned());
        }
        false
    })
    .ok()?;
    // `.dx/bazelrc` is bazelignored but the workspace rc imports it.
    let dx_rc = format!("{}/bazelrc", dx_path::state::DX_DIR);
    if workspace.join(&dx_rc).is_file() {
        inputs.push(dx_rc);
    }
    inputs.sort();
    inputs.dedup();
    let mut text = format!("{FORMAT}\n{}\n", argv.join("\0"));
    for rel in inputs {
        let digest = dx_digest::sha256_file_hex(&workspace.join(&rel)).ok()?;
        text.push_str(&format!("{digest} {rel}\n"));
    }
    Some(dx_digest::sha256_hex(text.as_bytes()))
}

/// The cached query output when the file holds `key`.
fn read_cached(path: &Path, key: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let (head, body) = bytes.split_at_checked(key.len() + 1)?;
    (head.strip_suffix(b"\n")? == key.as_bytes()).then(|| body.to_vec())
}

impl QueryRunner for PersistentGraphQuery<'_> {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        if !is_graph_query(argv) {
            return self.inner.run_query(argv, cwd);
        }
        let key = cache_key(argv, cwd);
        let path = cache_path(cwd);
        if let Some(stdout) = key.as_deref().and_then(|key| read_cached(&path, key)) {
            return Ok(QueryResult {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            });
        }
        let result = self.inner.run_query(argv, cwd)?;
        if let Some(key) = key
            && result.code == Some(0)
        {
            let mut content = Vec::with_capacity(key.len() + 1 + result.stdout.len());
            content.extend_from_slice(key.as_bytes());
            content.push(b'\n');
            content.extend_from_slice(&result.stdout);
            // A cache that cannot be written only costs the next run a query.
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| dx_atomic_fs::write_atomic(&path, &content));
            drop(written);
        }
        Ok(result)
    }

    fn run_info(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        self.inner.run_info(argv, cwd)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::FakeQuery;
    use dx_test_scratch::write;

    fn graph_argv() -> Vec<String> {
        crate::resolve::rule_graph::query_argv(&[])
    }

    fn workspace() -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("dx-resolve-graph-cache-");
        write(scratch.path(), "MODULE.bazel", "module(name = \"w\")\n");
        write(scratch.path(), "pkg/BUILD.bazel", "");
        write(scratch.path(), "pkg/defs.bzl", "");
        write(scratch.path(), "pkg/a.py", "x = 1\n");
        scratch
    }

    fn answers() -> FakeQuery {
        FakeQuery::new(vec![
            FakeQuery::ok("one\n"),
            FakeQuery::ok("two\n"),
            FakeQuery::ok("three\n"),
        ])
    }

    #[test]
    fn a_warm_run_skips_the_query_until_a_graph_input_changes() {
        let scratch = workspace();
        let root = scratch.path();
        let fake = answers();
        let argv = graph_argv();
        let first = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        // A fresh runner, as in the next dx command.
        let warm = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(first.stdout, b"one\n");
        assert_eq!(warm.stdout, b"one\n");
        assert_eq!(warm.code, Some(0));
        assert_eq!(fake.calls().len(), 1);

        write(root, "pkg/a.py", "x = 2\n");
        PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(fake.calls().len(), 1, "a source edit keeps the graph");

        for (rel, text) in [
            ("pkg/BUILD.bazel", "# edit\n"),
            ("pkg/defs.bzl", "# edit\n"),
        ] {
            let before = fake.calls().len();
            write(root, rel, text);
            let got = PersistentGraphQuery::new(&fake)
                .run_query(&argv, root)
                .unwrap();
            assert_eq!(fake.calls().len(), before + 1, "{rel} invalidates");
            assert_ne!(got.stdout, b"one\n");
        }
    }

    #[test]
    fn other_argv_and_failures_are_not_cached() {
        let scratch = workspace();
        let root = scratch.path();
        let fake = FakeQuery::new(vec![
            FakeQuery::failed("boom"),
            FakeQuery::ok("fine\n"),
            FakeQuery::ok("again\n"),
        ]);
        let mut argv = graph_argv();
        assert_eq!(
            PersistentGraphQuery::new(&fake)
                .run_query(&argv, root)
                .unwrap()
                .code,
            Some(2)
        );
        PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        argv.insert(1, "--output_base=/o".to_owned());
        let got = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(
            got.stdout, b"again\n",
            "startup options are part of the key"
        );
        assert_eq!(fake.calls().len(), 3);
    }
}
