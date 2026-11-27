use std::cell::Cell;
use std::hint::black_box;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use dx_adopt::{ChangeKind, GitChange};
use dx_plan::resolve::{PersistentGraphQuery, QueryResult, QueryRunner, resolve_affected};
use dx_testing::cli::{graph_rule, graph_rule_of_kind, graph_stdout};

const PACKAGES: usize = 200;
const ROUNDS: u32 = 20;
const CHANGED: usize = 7;

struct CountingQuery {
    graph: Vec<u8>,
    calls: Cell<usize>,
}

impl QueryRunner for CountingQuery {
    fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        self.calls.set(self.calls.get() + 1);
        Ok(QueryResult {
            code: Some(0),
            stdout: self.graph.clone(),
            stderr: Vec::new(),
        })
    }
}

fn graph() -> Vec<u8> {
    let mut lines = Vec::with_capacity(PACKAGES * 2);
    for n in 0..PACKAGES {
        let lib = format!("//pkg_{n}:lib");
        lines.push(graph_rule(&lib, &[&format!("//pkg_{n}:lib.txt")], &[]));
        lines.push(graph_rule_of_kind(
            &format!("//pkg_{n}:lib_test"),
            "sh_test",
            &[&lib],
            &[],
        ));
    }
    graph_stdout(&lines).into_bytes()
}

fn workspace(root: &Path) -> Result<(), String> {
    dx_test_scratch::write(root, "MODULE.bazel", "module(name = \"bench\")\n")?;
    for n in 0..PACKAGES {
        dx_test_scratch::write(root, &format!("pkg_{n}/BUILD.bazel"), "")?;
        dx_test_scratch::write(root, &format!("pkg_{n}/lib.txt"), "x\n")?;
    }
    Ok(())
}

fn select(root: &Path, bazel: &CountingQuery) -> Result<Vec<String>, String> {
    let changes = [GitChange {
        path: format!("pkg_{CHANGED}/lib.txt"),
        from: None,
        kind: ChangeKind::Modified,
    }];
    let runner = PersistentGraphQuery::new(bazel);
    let aspects = dx_plan::plan::registry::all_aspect_labels();
    let mut selection = resolve_affected(&changes, root, &runner, &[], &aspects, None)
        .map_err(|error| error.to_string())?;
    let graph = selection
        .graph
        .take()
        .ok_or("the affected selection kept no rule graph")?;
    Ok(graph.affected_tests(&selection.targets, &graph.affected(&selection.targets)))
}

fn timed_select(root: &Path, bazel: &CountingQuery, queries: usize) -> Result<Duration, String> {
    let before = bazel.calls.get();
    let start = Instant::now();
    let tests = black_box(select(root, bazel)?);
    let elapsed = start.elapsed();
    let ran = bazel.calls.get() - before;
    if ran != queries {
        return Err(format!("expected {queries} Bazel queries, ran {ran}"));
    }
    let want = [format!("//pkg_{CHANGED}:lib_test")];
    if tests != want {
        return Err(format!("expected tests {want:?}, got {tests:?}"));
    }
    Ok(elapsed)
}

fn report(name: &str, per_iter: Duration) {
    println!(
        "test {name} ... bench: {} ns/iter (+/- 0)",
        per_iter.as_nanos()
    );
}

fn run() -> Result<(), String> {
    let scratch = dx_test_scratch::scratch("dx-affected-bench-")?;
    let root = scratch.path();
    workspace(root)?;
    let bazel = CountingQuery {
        graph: graph(),
        calls: Cell::new(0),
    };
    report("affected_tests_cold", timed_select(root, &bazel, 1)?);

    let mut warm = Duration::ZERO;
    for _ in 0..ROUNDS {
        warm += timed_select(root, &bazel, 0)?;
    }
    report("affected_tests_warm", warm / ROUNDS);

    dx_test_scratch::write(root, "pkg_0/BUILD.bazel", "# edit\n")?;
    report("affected_tests_build_edit", timed_select(root, &bazel, 1)?);
    timed_select(root, &bazel, 0)?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("affected_bench: {error}");
            ExitCode::FAILURE
        }
    }
}
