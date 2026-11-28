use std::io;
use std::path::{Path, PathBuf};

use super::rule_graph::is_graph_query;
use super::{QueryResult, QueryRunner};

const FORMAT: &str = "dx-rule-graph-v2";

pub struct PersistentGraphQuery<'a> {
    inner: &'a dyn QueryRunner,
}

impl<'a> PersistentGraphQuery<'a> {
    pub fn new(inner: &'a dyn QueryRunner) -> Self {
        PersistentGraphQuery { inner }
    }
}

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

fn cache_key(argv: &[String], workspace: &Path) -> Option<String> {
    let mut inputs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let local = format!("{}/", dx_path::state::LOCAL_DIR);
    dx_adopt::load_graph::walk_workspace(workspace, &[".git"], |name, rel, _| {
        if is_graph_input(name) {
            inputs.push(rel.to_owned());
        } else if !rel.starts_with(&local) {
            files.push(rel.to_owned());
        }
        false
    })
    .ok()?;
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
    files.sort();
    for rel in files {
        text.push_str(&format!("- {rel}\n"));
    }
    Some(dx_digest::sha256_hex(text.as_bytes()))
}

pub struct GraphCache {
    path: PathBuf,
    key: Option<String>,
}

impl GraphCache {
    pub fn new(workspace: &Path, file: &str, argv: &[String]) -> Self {
        GraphCache {
            path: dx_path::state::local_dir(workspace).join(file),
            key: cache_key(argv, workspace),
        }
    }

    pub fn read(&self) -> Option<Vec<u8>> {
        let key = self.key.as_deref()?;
        let bytes = std::fs::read(&self.path).ok()?;
        let (head, body) = bytes.split_at_checked(key.len() + 1)?;
        (head.strip_suffix(b"\n")? == key.as_bytes()).then(|| body.to_vec())
    }

    pub fn write(&self, body: &[u8]) {
        let Some(key) = self.key.as_deref() else {
            return;
        };
        let mut content = Vec::with_capacity(key.len() + 1 + body.len());
        content.extend_from_slice(key.as_bytes());
        content.push(b'\n');
        content.extend_from_slice(body);
        let written = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| dx_atomic_fs::write_atomic(&self.path, &content));
        drop(written);
    }
}

impl QueryRunner for PersistentGraphQuery<'_> {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        if !is_graph_query(argv) {
            return self.inner.run_query(argv, cwd);
        }
        let cache = GraphCache::new(cwd, dx_path::state::RULE_GRAPH, argv);
        if let Some(stdout) = cache.read() {
            return Ok(QueryResult {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            });
        }
        let result = self.inner.run_query(argv, cwd)?;
        if result.code == Some(0) {
            cache.write(&result.stdout);
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
        crate::resolve::rule_graph::query_argv(Path::new("/w"), &[])
    }

    fn workspace() -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("dx-resolve-graph-cache-").unwrap();
        write(scratch.path(), "MODULE.bazel", "module(name = \"w\")\n").unwrap();
        write(scratch.path(), "pkg/BUILD.bazel", "").unwrap();
        write(scratch.path(), "pkg/defs.bzl", "").unwrap();
        write(scratch.path(), "pkg/a.py", "x = 1\n").unwrap();
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
        let warm = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(first.stdout, b"one\n");
        assert_eq!(warm.stdout, b"one\n");
        assert_eq!(warm.code, Some(0));
        assert_eq!(fake.calls().len(), 1);

        write(root, "pkg/a.py", "x = 2\n").unwrap();
        PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(fake.calls().len(), 1, "a source edit keeps the graph");

        for (rel, text) in [
            ("pkg/BUILD.bazel", "# edit\n"),
            ("pkg/defs.bzl", "# edit\n"),
        ] {
            let before = fake.calls().len();
            write(root, rel, text).unwrap();
            let got = PersistentGraphQuery::new(&fake)
                .run_query(&argv, root)
                .unwrap();
            assert_eq!(fake.calls().len(), before + 1, "{rel} invalidates");
            assert_ne!(got.stdout, b"one\n");
        }
    }

    #[test]
    fn adding_or_deleting_a_source_file_invalidates() {
        let scratch = workspace();
        let root = scratch.path();
        let fake = answers();
        let argv = graph_argv();
        PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(fake.calls().len(), 1);

        write(root, "pkg/new.py", "y = 1\n").unwrap();
        let added = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(fake.calls().len(), 2, "an added file can match a glob");
        assert_eq!(added.stdout, b"two\n");

        std::fs::remove_file(root.join("pkg/new.py")).unwrap();
        let deleted = PersistentGraphQuery::new(&fake)
            .run_query(&argv, root)
            .unwrap();
        assert_eq!(fake.calls().len(), 3, "a deleted file can leave a glob");
        assert_eq!(deleted.stdout, b"three\n");
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
