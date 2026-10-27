use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use super::rule_graph::is_graph_query;
use super::{QueryResult, QueryRunner};

/// Query runner that remembers the successful workspace rule-graph query.
///
/// Only the `kind('rule', //...)` query is cached, keyed on the exact argv and
/// cwd. Launch errors and non-zero exits are never cached. Callers must call
/// `invalidate` after anything that may have written BUILD files.
pub struct MemoizedGraphQuery<'a> {
    inner: &'a dyn QueryRunner,
    cache: RefCell<Vec<(Vec<String>, PathBuf, QueryResult)>>,
}

impl<'a> MemoizedGraphQuery<'a> {
    pub fn new(inner: &'a dyn QueryRunner) -> Self {
        MemoizedGraphQuery {
            inner,
            cache: RefCell::new(Vec::new()),
        }
    }

    /// Drops every cached graph so the next graph query reaches the inner runner.
    pub fn invalidate(&self) {
        self.cache.borrow_mut().clear();
    }
}

impl QueryRunner for MemoizedGraphQuery<'_> {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        if !is_graph_query(argv) {
            return self.inner.run_query(argv, cwd);
        }
        let hit = self
            .cache
            .borrow()
            .iter()
            .find(|(key, dir, _)| key == argv && dir == cwd)
            .map(|(_, _, result)| result.clone());
        if let Some(result) = hit {
            return Ok(result);
        }
        let result = self.inner.run_query(argv, cwd)?;
        if result.code == Some(0) {
            self.cache
                .borrow_mut()
                .push((argv.to_vec(), cwd.to_path_buf(), result.clone()));
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

    fn graph_argv(startup: &[&str]) -> Vec<String> {
        let mut argv: Vec<String> = startup.iter().map(|s| (*s).to_owned()).collect();
        argv.push("bazel".to_owned());
        argv.extend(
            [
                "query",
                "--keep_going",
                "--output=streamed_jsonproto",
                "--proto:output_rule_attrs=tags",
                "--noproto:default_values",
                "--noproto:locations",
                "--noproto:instantiation_stack",
                "--",
                "kind('rule', //...)",
            ]
            .map(str::to_owned),
        );
        argv
    }

    fn other_argv() -> Vec<String> {
        vec![
            "bazel".to_owned(),
            "query".to_owned(),
            "--".to_owned(),
            "//a:all".to_owned(),
        ]
    }

    #[test]
    fn repeated_graph_query_reaches_inner_once() {
        let fake = FakeQuery::new(vec![FakeQuery::ok("one")]);
        let memo = MemoizedGraphQuery::new(&fake);
        let argv = graph_argv(&[]);
        let first = memo.run_query(&argv, Path::new("/w")).unwrap();
        let second = memo.run_query(&argv, Path::new("/w")).unwrap();
        assert_eq!(first, second);
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn different_options_or_cwd_miss() {
        let fake = FakeQuery::new(vec![
            FakeQuery::ok("a"),
            FakeQuery::ok("b"),
            FakeQuery::ok("c"),
        ]);
        let memo = MemoizedGraphQuery::new(&fake);
        memo.run_query(&graph_argv(&[]), Path::new("/w")).unwrap();
        memo.run_query(&graph_argv(&["--x"]), Path::new("/w"))
            .unwrap();
        memo.run_query(&graph_argv(&[]), Path::new("/v")).unwrap();
        assert_eq!(fake.calls().len(), 3);
    }

    #[test]
    fn other_queries_pass_through() {
        let fake = FakeQuery::new(vec![FakeQuery::ok("a"), FakeQuery::ok("b")]);
        let memo = MemoizedGraphQuery::new(&fake);
        memo.run_query(&other_argv(), Path::new("/w")).unwrap();
        memo.run_query(&other_argv(), Path::new("/w")).unwrap();
        assert_eq!(fake.calls().len(), 2);
    }

    #[test]
    fn failures_are_not_cached() {
        let fake = FakeQuery::new(vec![FakeQuery::failed("boom"), FakeQuery::ok("fine")]);
        let memo = MemoizedGraphQuery::new(&fake);
        let argv = graph_argv(&[]);
        assert_eq!(
            memo.run_query(&argv, Path::new("/w")).unwrap().code,
            Some(2)
        );
        assert_eq!(
            memo.run_query(&argv, Path::new("/w")).unwrap().code,
            Some(0)
        );
        assert_eq!(fake.calls().len(), 2);
    }

    struct Failing;

    impl QueryRunner for Failing {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Err(io::Error::other("launch failed"))
        }

        fn run_info(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Ok(QueryResult {
                code: Some(0),
                stdout: b"info".to_vec(),
                stderr: Vec::new(),
            })
        }

        fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
            Some(7)
        }
    }

    #[test]
    fn launch_errors_pass_through_and_delegation_is_exact() {
        let memo = MemoizedGraphQuery::new(&Failing);
        assert!(memo.run_query(&graph_argv(&[]), Path::new("/w")).is_err());
        assert!(memo.run_query(&graph_argv(&[]), Path::new("/w")).is_err());
        assert_eq!(memo.free_disk_bytes(Path::new("/w")), Some(7));
        let info = memo.run_info(&other_argv(), Path::new("/w")).unwrap();
        assert_eq!(info.stdout, b"info");
    }

    #[test]
    fn invalidate_forces_reload() {
        let fake = FakeQuery::new(vec![FakeQuery::ok("a"), FakeQuery::ok("b")]);
        let memo = MemoizedGraphQuery::new(&fake);
        let argv = graph_argv(&[]);
        memo.run_query(&argv, Path::new("/w")).unwrap();
        memo.invalidate();
        let again = memo.run_query(&argv, Path::new("/w")).unwrap();
        assert_eq!(again.stdout, b"b");
        assert_eq!(fake.calls().len(), 2);
    }
}
