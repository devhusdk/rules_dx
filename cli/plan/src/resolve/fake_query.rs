use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use dx_testing::cli::graph_stdout;

use crate::resolve::{QueryResult, QueryRunner};

struct Call {
    argv: Vec<String>,
    cwd: PathBuf,
    expression: String,
}

pub(crate) struct FakeQuery {
    calls: RefCell<Vec<Call>>,
    outputs: RefCell<Vec<QueryResult>>,
}

impl FakeQuery {
    pub(crate) fn new(outputs: Vec<QueryResult>) -> Self {
        FakeQuery {
            calls: RefCell::new(Vec::new()),
            outputs: RefCell::new(outputs),
        }
    }

    pub(crate) fn ok(lines: &str) -> QueryResult {
        FakeQuery::raw(lines.as_bytes().to_vec())
    }

    pub(crate) fn graph(lines: &[String]) -> QueryResult {
        FakeQuery::ok(&graph_stdout(lines))
    }

    pub(crate) fn failed(stderr: &str) -> QueryResult {
        QueryResult {
            code: Some(2),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    pub(crate) fn raw(stdout: Vec<u8>) -> QueryResult {
        QueryResult {
            code: Some(0),
            stdout,
            stderr: Vec::new(),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(Vec<String>, PathBuf)> {
        self.calls
            .borrow()
            .iter()
            .map(|call| (call.argv.clone(), call.cwd.clone()))
            .collect()
    }

    pub(crate) fn expressions(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|call| call.expression.clone())
            .collect()
    }

    pub(crate) fn verbs(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|call| {
                call.argv
                    .iter()
                    .find(|arg| *arg == "query" || *arg == "cquery")
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }
}

impl QueryRunner for FakeQuery {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        let expression = match argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--query_file="))
        {
            Some(path) => std::fs::read_to_string(path)?,
            None => argv.last().cloned().unwrap_or_default(),
        };
        self.calls.borrow_mut().push(Call {
            argv: argv.to_vec(),
            cwd: cwd.to_path_buf(),
            expression,
        });
        Ok(self.outputs.borrow_mut().remove(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_testing::cli::{graph_rule, strings};

    #[test]
    fn results_replay_in_queue_order() {
        let query = FakeQuery::new(vec![FakeQuery::ok("first"), FakeQuery::ok("second")]);
        let first = query
            .run_query(&strings(&["a"]), Path::new("/w"))
            .expect("first");
        let second = query
            .run_query(&strings(&["b"]), Path::new("/w"))
            .expect("second");
        assert_eq!(first.stdout, b"first");
        assert_eq!(second.stdout, b"second");
    }

    #[test]
    #[should_panic(expected = "")]
    fn empty_queue_panics() {
        let query = FakeQuery::new(Vec::new());
        drop(query.run_query(&strings(&["a"]), Path::new("/w")));
    }

    #[test]
    fn calls_record_argv_and_cwd() {
        let query = FakeQuery::new(vec![FakeQuery::ok("")]);
        query
            .run_query(&strings(&["query", "//..."]), Path::new("/work"))
            .expect("query");
        assert_eq!(
            query.calls(),
            vec![(strings(&["query", "//..."]), PathBuf::from("/work"))]
        );
    }

    #[test]
    fn expression_comes_from_query_file_or_last_argv() {
        let scratch = dx_test_scratch::scratch("dx-fake-query-expr-").unwrap();
        let file = scratch.path().join("q.txt");
        std::fs::write(&file, "deps(//a)").expect("query file");
        let query = FakeQuery::new(vec![FakeQuery::ok(""), FakeQuery::ok("")]);
        let from_file = vec![
            "query".to_owned(),
            format!("--query_file={}", file.display()),
        ];
        query.run_query(&from_file, scratch.path()).expect("file");
        query
            .run_query(&strings(&["cquery", "//b"]), scratch.path())
            .expect("argv");
        assert_eq!(query.expressions(), strings(&["deps(//a)", "//b"]));
        assert_eq!(query.verbs(), strings(&["query", "cquery"]));
    }

    #[test]
    fn missing_query_file_is_an_io_error() {
        let query = FakeQuery::new(vec![FakeQuery::ok("")]);
        let argv = strings(&["query", "--query_file=/dx/no/such/query/file"]);
        assert!(query.run_query(&argv, Path::new("/w")).is_err());
        assert_eq!(query.calls(), [] as [(Vec<String>, PathBuf); 0]);
    }

    #[test]
    fn result_constructors_set_codes_and_streams() {
        let ok = FakeQuery::ok("out");
        assert_eq!(
            (ok.code, ok.stdout, ok.stderr),
            (Some(0), b"out".to_vec(), Vec::new())
        );
        let failed = FakeQuery::failed("err");
        assert_eq!(
            (failed.code, failed.stdout, failed.stderr),
            (Some(2), Vec::new(), b"err".to_vec())
        );
        let raw = FakeQuery::raw(vec![0xff]);
        assert_eq!((raw.code, raw.stdout), (Some(0), vec![0xff]));
        let rule = graph_rule("//a:a", &[], &[]);
        let graph = FakeQuery::graph(std::slice::from_ref(&rule));
        assert_eq!(graph.stdout, format!("{rule}\n").into_bytes());
    }
}
