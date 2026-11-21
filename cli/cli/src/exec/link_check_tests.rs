use dx_exec_quality_testing::{online_link_request, serve_links};
use serde_json::json;
use std::path::Path;

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
    use super::test_support::Harness;
    let lychee =
        std::path::absolute(dx_testing::env_file("DX_TEST_LYCHEE")).expect("absolute lychee path");
    let base = serve_links();
    let mut harness = Harness::new("lint-online");
    harness.write_source(
        "docs/README.md",
        &format!("# Links\n\n[up]({base}/ok)\n\n[gone]({base}/missing)\n"),
    );
    let request = harness.temp.join("readme-request.json");
    std::fs::write(&request, online_link_request(&lychee)).unwrap();
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
    use super::test_support::Harness;
    let mut harness = Harness::new("lint-offline");
    let request = harness.temp.join("readme-request.json");
    std::fs::write(&request, b"{").unwrap();
    harness.raw_bep = Some(link_request_bep(&request));
    let (_, stdout, stderr) = harness.run(&["lint", "--output=json"]);
    assert!(!stdout.contains("link check"), "{stdout}\n{stderr}");
    assert!(!stderr.contains("link check"), "{stdout}\n{stderr}");
}
