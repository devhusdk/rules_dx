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

fn serve_links() -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let read = stream.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..read]);
            let status = if head.contains(" /ok ") {
                "200 OK"
            } else {
                "404 Not Found"
            };
            let _ = stream.write_all(
                format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            );
        }
    });
    base
}

fn online_request(lychee: &Path) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "producer": "//docs:readme",
        "capability": "lint",
        "stages": [
            {"tool": "lychee", "classes": ["markdown"], "sources": ["docs/README.md"]},
            {"tool": "typos", "classes": ["markdown"], "sources": ["docs/README.md"]},
        ],
        "sources": [{"workspace": "docs/README.md", "exec": "docs/README.md"}],
        "tools": {
            "lychee": {"binary": lychee.to_string_lossy()},
            "typos": {"binary": "external/typos+/typos"},
        },
    }))
    .unwrap()
}

fn broken_links(result: &[u8]) -> Vec<String> {
    quality_result::decode_validated(result)
        .expect("valid result")
        .initial_diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.tool_id == TOOL)
        .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
        .collect()
}

#[test]
fn online_replay_reports_a_broken_link_on_a_local_server() {
    let lychee =
        std::path::absolute(dx_testing::env_file("DX_TEST_LYCHEE")).expect("absolute lychee path");
    let base = serve_links();
    let exec = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(exec.path().join("docs")).unwrap();
    std::fs::write(
        exec.path().join("docs/README.md"),
        format!("# Links\n\n[up]({base}/ok)\n\n[gone]({base}/missing)\n"),
    )
    .unwrap();
    let roots = Roots {
        output_base: exec.path().join("output_base"),
        execution_root: exec.path().to_path_buf(),
    };
    let scratch = tempfile::tempdir().unwrap();
    let requests = vec![(
        "docs/readme-request.json".to_owned(),
        online_request(&lychee),
    )];
    let outcome = run(
        &requests,
        &roots,
        scratch.path(),
        "e2e",
        &|request, output| {
            quality_runner::entry::run_request(request, output, Some("link-check"))
                .map_err(|error| error.to_string())
        },
    );
    assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
    assert_eq!(outcome.results.len(), 1);
    let found = broken_links(&outcome.results[0]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("docs/README.md: "), "{found:?}");
    assert!(found[0].contains(&format!("{base}/missing")), "{found:?}");
}

fn link_request_bep(request: &Path) -> Vec<String> {
    vec![
        json!({
            "id": {"namedSet": {"id": "0"}},
            "namedSetOfFiles": {"files": [{"uri": format!("file://{}", request.display())}]},
        })
        .to_string(),
        json!({
            "id": {"targetCompleted": {"label": "//docs:readme"}},
            "completed": {
                "success": true,
                "outputGroup": [
                    {"name": dx_plan::plan::LINK_REQUEST_OUTPUT_GROUP, "fileSets": [{"id": "0"}]},
                ],
            },
        })
        .to_string(),
    ]
}

#[test]
fn lint_online_reports_a_broken_link_from_the_build_events() {
    use super::super::test_support::Harness;
    let lychee =
        std::path::absolute(dx_testing::env_file("DX_TEST_LYCHEE")).expect("absolute lychee path");
    let base = serve_links();
    let mut harness = Harness::new("lint-online");
    harness.write_source(
        "docs/README.md",
        &format!("# Links\n\n[up]({base}/ok)\n\n[gone]({base}/missing)\n"),
    );
    let request = harness.temp.join("readme-request.json");
    std::fs::write(&request, online_request(&lychee)).unwrap();
    harness.raw_bep = Some(link_request_bep(&request));
    harness
        .query
        .script_info(&format!("{}\n", harness.workspace.display()));
    let (code, stdout, stderr) = harness.run(&["lint", "--online", "--output=json"]);
    assert_ne!(code, 0, "{stdout}\n{stderr}");
    assert!(
        stdout.contains(&format!("{base}/missing")),
        "{stdout}\n{stderr}"
    );
    assert!(
        !stdout.contains(&format!("{base}/ok")),
        "{stdout}\n{stderr}"
    );
    assert!(
        harness
            .query
            .info_calls
            .borrow()
            .iter()
            .any(|argv| argv.ends_with(&["info".to_owned(), "execution_root".to_owned()])),
        "{stdout}\n{stderr}"
    );
}

#[test]
fn lint_without_online_leaves_the_link_requests_alone() {
    use super::super::test_support::Harness;
    let mut harness = Harness::new("lint-offline");
    let request = harness.temp.join("readme-request.json");
    std::fs::write(&request, b"{").unwrap();
    harness.raw_bep = Some(link_request_bep(&request));
    let (_, stdout, stderr) = harness.run(&["lint", "--output=json"]);
    assert!(!stdout.contains("link check"), "{stdout}\n{stderr}");
    assert!(!stderr.contains("link check"), "{stdout}\n{stderr}");
}
