use super::*;
use dx_process::Scope;
use dx_test_scratch::write;
use dx_testing::cli::strings;

fn scope(targets: &[&str]) -> ResolvedScope {
    ResolvedScope {
        scope: Scope::ResolvedOwners(strings(targets)),
        targets: strings(targets),
        excluded: Vec::new(),
        kept: Vec::new(),
    }
}

#[test]
fn apply_drops_covered_labels_and_subtracts_patterns() {
    let exclude = strings(&["//third_party/...", "//vendor:blob"]);
    // (targets, typed scopes, targets after, excluded, kept)
    let cases: [(&[&str], &[&str], &[&str], &[&str], &[&str]); 5] = [
        (
            &["//..."],
            &[],
            &["//..."],
            &["//third_party/...", "//vendor:blob"],
            &[],
        ),
        (
            &["//app:bin", "//third_party/x:y"],
            &["app/main.rs"],
            &["//app:bin"],
            &[],
            &[],
        ),
        (
            &["//app/...", "//third_party/x:y"],
            &["//app/...", "//third_party/x:y"],
            &["//app/...", "//third_party/x:y"],
            &[],
            &["//third_party/x:y"],
        ),
        (
            &["//third_party/..."],
            &["//third_party/..."],
            &["//third_party/..."],
            &[],
            &["//third_party/..."],
        ),
        (
            &["//vendor:all"],
            &[],
            &["//vendor:all"],
            &["//vendor:blob"],
            &[],
        ),
    ];
    for (targets, typed, after, excluded, kept) in cases {
        let got = apply(scope(targets), &exclude, &strings(typed)).expect("apply");
        assert_eq!(got.targets, strings(after), "{targets:?}");
        assert_eq!(got.excluded, strings(excluded), "{targets:?}");
        assert_eq!(got.kept, strings(kept), "{targets:?}");
    }
}

#[test]
fn no_excludes_leave_the_scope_alone() {
    let before = scope(&["//third_party/x:y"]);
    assert_eq!(apply(before.clone(), &[], &[]).expect("apply"), before);
}

#[test]
fn an_external_repo_target_is_never_excluded() {
    let got = apply(
        scope(&["@dep//:lib"]),
        &strings(&["//third_party/..."]),
        &[],
    )
    .expect("apply");
    assert_eq!(got.targets, strings(&["@dep//:lib"]));
    assert!(got.excluded.is_empty());
}

#[test]
fn excluding_everything_fails_instead_of_widening_to_the_repo() {
    let err = apply(
        scope(&["//third_party/..."]),
        &strings(&["//third_party/..."]),
        &strings(&["third_party"]),
    )
    .expect_err("all excluded");
    assert_eq!(
        err,
        ResolveError::AllExcluded {
            scopes: strings(&["third_party"])
        }
    );
}

#[test]
fn bazel_labels_put_negatives_before_kept_targets() {
    let resolved = ResolvedScope {
        excluded: strings(&["//third_party/..."]),
        kept: strings(&["//third_party/x:y"]),
        ..scope(&["//...", "//third_party/x:y"])
    };
    assert_eq!(
        bazel_labels(&resolved),
        strings(&["//...", "-//third_party/...", "//third_party/x:y"])
    );
    assert_eq!(
        query_set(&resolved),
        r#"((set("//...") except set("//third_party/...")) + set("//third_party/x:y"))"#
    );
    assert_eq!(query_set(&scope(&["//a:b"])), r#"set("//a:b")"#);
}

#[test]
fn load_reads_dx_toml_and_rejects_a_bad_pattern() {
    let scratch = dx_test_scratch::scratch("dx-resolve-exclude-");
    let root = scratch.path();
    assert_eq!(load(root).expect("missing file"), Vec::<String>::new());
    write(
        root,
        "dx.toml",
        "schema_version = 1\n[scope]\nexclude = [\"//third_party/...\"]\n",
    );
    assert_eq!(load(root).expect("load"), strings(&["//third_party/..."]));
    write(
        root,
        "dx.toml",
        "schema_version = 1\n[scope]\nexclude = [\"third_party\"]\n",
    );
    let err = load(root).expect_err("bad pattern");
    assert!(
        matches!(&err, ResolveError::Config { message } if message.contains("third_party")),
        "{err}"
    );
    assert!(err.to_string().starts_with("invalid dx.toml: "), "{err}");
}

#[test]
fn resolve_applies_dx_toml_excludes_for_every_entry_point() {
    let scratch = dx_test_scratch::scratch("dx-resolve-exclude-entry-");
    let root = scratch.path();
    write(
        root,
        "dx.toml",
        "schema_version = 1\n[scope]\nexclude = [\"//third_party/...\"]\n",
    );
    let query = super::super::NeverQuery;
    let whole = super::super::resolve(&[], root, &query, &[]).expect("resolve");
    assert_eq!(whole.targets, strings(&["//..."]));
    assert_eq!(whole.excluded, strings(&["//third_party/..."]));
    let tests = super::super::resolve_for_test_with_selection(
        &[],
        root,
        &query,
        &[],
        &super::super::SelectionContext::unconfigured(&[]),
    )
    .expect("resolve");
    assert_eq!(tests, whole);
    let named = super::super::resolve(&strings(&["//third_party/x:y"]), root, &query, &[])
        .expect("resolve");
    assert_eq!(named.kept, strings(&["//third_party/x:y"]));
}
