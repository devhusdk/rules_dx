use super::*;
use serde_json::json;

fn roots() -> Roots {
    Roots {
        output_base: PathBuf::from("/out"),
        execution_root: PathBuf::from("/exec"),
    }
}

fn request() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "producer": "//pkg:docs",
        "capability": "lint",
        "stages": [
            {"tool": "lychee", "classes": ["markdown"], "sources": ["README.md"]},
            {"tool": "typos", "classes": ["markdown"], "sources": ["README.md", "NOTES.md"]},
        ],
        "sources": [
            {"workspace": "README.md", "exec": "README.md"},
            {"workspace": "NOTES.md", "exec": "NOTES.md"},
        ],
        "siblings": [{"workspace": "a.md", "exec": "bazel-out/k8/bin/a.md"}],
        "resolves": [{"workspace": "b.md", "exec": "external/dep+/b.md"}],
        "tools": {
            "lychee": {
                "binary": "external/lychee+/lychee",
                "config": "lychee.toml",
                "files": [{"mirror_rel": "lychee.toml", "exec": "bazel-out/k8/bin/lychee.toml"}],
                "env": [{"key": "A", "value": "b"}],
                "upstream": ["bazel-out/k8/bin/up.bin"],
                "search_paths": ["external/site"],
                "path_tools": [],
            },
            "typos": {"binary": "external/typos+/typos"},
        },
    }))
    .unwrap()
}

#[test]
fn prepare_keeps_only_the_lychee_stage_and_goes_online() {
    let bytes = prepare(&request(), &roots(), "/scratch").unwrap().unwrap();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["stages"].as_array().unwrap().len(), 1);
    assert_eq!(doc["sources"].as_array().unwrap().len(), 1);
    assert_eq!(doc["tools"].as_object().unwrap().len(), 1);
    assert_eq!(doc["scratch_parent"], "/scratch");
    assert_eq!(
        doc["tools"]["lychee"]["env"],
        json!([{"key": "A", "value": "b"}, {"key": "DX_LINT_ONLINE", "value": "1"}])
    );
}

#[test]
fn prepare_makes_every_exec_path_absolute() {
    let bytes = prepare(&request(), &roots(), "/scratch").unwrap().unwrap();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();
    let tool = &doc["tools"]["lychee"];
    assert_eq!(doc["sources"][0]["exec"], "/exec/README.md");
    assert_eq!(doc["siblings"][0]["exec"], "/exec/bazel-out/k8/bin/a.md");
    assert_eq!(doc["resolves"][0]["exec"], "/out/external/dep+/b.md");
    assert_eq!(tool["binary"], "/out/external/lychee+/lychee");
    assert_eq!(
        tool["files"][0]["exec"],
        "/exec/bazel-out/k8/bin/lychee.toml"
    );
    assert_eq!(tool["upstream"][0], "/exec/bazel-out/k8/bin/up.bin");
    assert_eq!(tool["search_paths"][0], "/out/external/site");
    assert_eq!(tool["config"], "lychee.toml");
}

#[test]
fn prepare_leaves_absolute_paths_alone() {
    assert_eq!(roots().absolute("/abs/lychee"), "/abs/lychee");
}

#[test]
fn prepare_skips_a_request_without_lychee() {
    let bytes = serde_json::to_vec(&json!({
        "stages": [{"tool": "typos", "classes": [], "sources": []}],
    }))
    .unwrap();
    assert_eq!(prepare(&bytes, &roots(), "/scratch"), Ok(None));
}

#[test]
fn prepare_rejects_bad_json() {
    assert!(prepare(b"not json", &roots(), "/scratch").is_err());
}

#[test]
fn run_reports_a_failed_request_as_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let requests = vec![("pkg/a-request.json".to_owned(), request())];
    let outcome = run(&requests, &roots(), dir.path(), "t", &|_, _| {
        Err("boom".to_owned())
    });
    assert!(outcome.results.is_empty());
    assert_eq!(outcome.problems.len(), 1);
    assert!(outcome.problems[0].contains("pkg/a-request.json"));
    assert!(outcome.problems[0].contains("boom"));
}

#[test]
fn run_returns_the_runner_output_and_removes_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let requests = vec![("a".to_owned(), request())];
    let outcome = run(&requests, &roots(), dir.path(), "t", &|request, output| {
        assert!(std::fs::read(request).is_ok());
        std::fs::write(output, b"result").map_err(|error| error.to_string())
    });
    assert_eq!(outcome.results, vec![b"result".to_vec()]);
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn run_names_a_malformed_request() {
    let dir = tempfile::tempdir().unwrap();
    let requests = vec![("a".to_owned(), b"{".to_vec())];
    let outcome = run(&requests, &roots(), dir.path(), "t", &|_, _| Ok(()));
    assert_eq!(outcome.problems.len(), 1);
}
