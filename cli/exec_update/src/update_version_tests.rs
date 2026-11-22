use super::*;

fn tag_request() -> dx_update::edit::UpdateRequest {
    dx_update::edit::UpdateRequest::parse("github-actions:actions/checkout", "v5").expect("request")
}

#[test]
fn gha_tag_is_resolved_through_the_fetch() {
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let seen = std::cell::RefCell::new(Vec::new());
    let fetch = |url: &str| {
        seen.borrow_mut().push(url.to_owned());
        Ok(format!("{{\"sha\": \"{sha}\"}}"))
    };
    let resolved = resolve_gha_tag_request(&tag_request(), None, false, &fetch).expect("resolved");
    assert_eq!(
        *seen.borrow(),
        ["https://api.github.com/repos/actions/checkout/commits/v5"]
    );
    let text = "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v4\n";
    let widened = resolved.plan_edit(text).expect("edit");
    assert!(widened.ends_with(&format!("@{sha} # v5\n")), "{widened}");
}

#[test]
fn gha_tag_fetch_error_and_offline_resolve_nothing() {
    let failing = |_: &str| Err(DxError::failure("HTTP 404"));
    let (code, message) =
        resolve_gha_tag_request(&tag_request(), None, false, &failing).expect_err("error");
    assert_eq!(code, CODE_UPDATE_WIDEN_FAILED);
    assert!(message.message().contains("HTTP 404"), "{message}");
    let calls = std::cell::Cell::new(0);
    let counting = |_: &str| {
        calls.set(calls.get() + 1);
        Err(DxError::failure("unused"))
    };
    let (code, _) =
        resolve_gha_tag_request(&tag_request(), None, true, &counting).expect_err("error");
    assert_eq!(code, CODE_OFFLINE_REQUIRED);
    assert_eq!(calls.get(), 0);
}
