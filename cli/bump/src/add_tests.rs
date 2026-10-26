use super::*;
use crate::request::BumpPlanOutcome;

fn add(selector: &str, version: &str, content: &str) -> Result<String, AddError> {
    AddRequest::parse(selector, version)
        .expect("request")
        .plan_insert(content)
}

/// `dx bump` reads what `dx add` wrote: the same version is current, a higher one widens.
fn assert_bump_round_trip(selector: &str, version: &str, higher: &str, written: &str) {
    let same = BumpRequest::parse(selector, version).expect("bump request");
    assert_eq!(
        same.plan_edit_outcome(written),
        Ok(BumpPlanOutcome::Unchanged),
        "{selector} at {version}"
    );
    let next = BumpRequest::parse(selector, higher).expect("bump request");
    assert!(
        matches!(
            next.plan_edit_outcome(written),
            Ok(BumpPlanOutcome::Changed(_))
        ),
        "{selector} to {higher}"
    );
}

fn assert_declared(selector: &str, content: &str, manifest: &str) {
    let error = add(selector, "9.9.9", content).expect_err("declared");
    assert_eq!(
        error,
        AddError::Declared {
            selector: selector.to_owned(),
            manifest: manifest.to_owned(),
        }
    );
    assert!(
        error
            .to_string()
            .contains(&format!("use `dx bump {selector} <version>`")),
        "{error}"
    );
}

#[test]
fn sets_are_the_bump_sets_without_github_actions() {
    let expected: Vec<BumpSet> = BumpSet::ALL
        .into_iter()
        .filter(|set| *set != BumpSet::GithubActions)
        .collect();
    assert_eq!(AddRequest::SETS.to_vec(), expected);
    assert_eq!(AddRequest::pipe_list(), "bazel|cargo|go|maven|npm|nuget");
}

#[test]
fn parse_rejects_bump_only_targets_with_a_hint() {
    for selector in ["github-actions:actions/checkout", "github-actions"] {
        let error = AddRequest::parse(selector, "v4").expect_err("rejected");
        assert!(matches!(error, AddError::UnsupportedSet { .. }), "{error}");
        assert!(
            error.to_string().contains("dx bump github-actions:"),
            "{error}"
        );
    }
    let error = AddRequest::parse("bazel:.bazelversion", "8.0.0").expect_err("rejected");
    assert!(
        error.to_string().contains("dx bump bazel:.bazelversion"),
        "{error}"
    );
    assert_eq!(AddRequest::parse("", "1.0.0"), Err(AddError::Empty));
    assert_eq!(AddRequest::parse("cargo:demo", ""), Err(AddError::Empty));
    assert_eq!(
        AddRequest::parse("cargo", "1.0.0"),
        Err(AddError::BareSet {
            set: "cargo".to_owned()
        })
    );
    assert!(matches!(
        AddRequest::parse("//foo:bar", "1.0.0"),
        Err(AddError::NotAPackage { .. })
    ));
    let unknown = AddRequest::parse("pip:demo", "1.0.0").expect_err("unknown");
    assert!(
        unknown
            .to_string()
            .contains("bazel|cargo|go|maven|npm|nuget"),
        "{unknown}"
    );
    assert!(matches!(
        AddRequest::parse("cargo:demo", "^1"),
        Err(AddError::Bump(BumpError::Version(_)))
    ));
    for message in [
        AddError::Empty.to_string(),
        unknown.to_string(),
        AddError::BareSet {
            set: "cargo".to_owned(),
        }
        .to_string(),
    ] {
        assert!(!message.contains("bump"), "{message}");
    }
}

#[test]
fn bazel_inserts_after_the_last_dep() {
    let before = "module(name = \"m\")\n\nbazel_dep(name = \"a\", version = \"1.0.0\")\n\nuse_repo(x, \"y\")\n";
    let after = add("bazel:demo", "1.2.3", before).expect("insert");
    assert_eq!(
        after,
        "module(name = \"m\")\n\nbazel_dep(name = \"a\", version = \"1.0.0\")\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n\nuse_repo(x, \"y\")\n"
    );
    assert_bump_round_trip("bazel:demo", "1.2.3", "2.0.0", &after);
    assert_declared("bazel:a", before, "MODULE.bazel");
    assert_declared(
        "bazel:a",
        "bazel_dep(name = \"a\", version = \"1\")\nbazel_dep(name = \"a\", version = \"1\")\n",
        "MODULE.bazel",
    );
    assert!(matches!(
        add("bazel:demo", "1.2.3", "bazel_dep(name = \"a\""),
        Err(AddError::UnsupportedManifest { .. })
    ));
}

#[test]
fn configured_bazel_sets_insert_into_their_manifest() {
    let request =
        AddRequest::parse_configured("tools", "tools/MODULE.bazel", "tools:demo", "1.2.3")
            .expect("request");
    assert_eq!(request.target_manifest(), "tools/MODULE.bazel");
    assert!(!request.needs_update_refresh());
    let after = request
        .plan_insert("module(name = \"t\")\n")
        .expect("insert");
    assert_eq!(
        after,
        "module(name = \"t\")\n\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n"
    );
    assert_eq!(
        request
            .plan_insert("bazel_dep(name = \"demo\", version = \"1.0.0\")\n")
            .expect_err("declared"),
        AddError::Declared {
            selector: "tools:demo".to_owned(),
            manifest: "tools/MODULE.bazel".to_owned(),
        }
    );
}

const MAVEN_HEAD: &str =
    "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n";

#[test]
fn maven_appends_to_the_default_install() {
    let before = format!(
        "{MAVEN_HEAD}maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n    ],\n)\n"
    );
    let after = add("maven:com.example:demo", "1.2.3", &before).expect("insert");
    assert_eq!(
        after,
        format!(
            "{MAVEN_HEAD}maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n        \"com.example:demo:1.2.3\",\n    ],\n)\n"
        )
    );
    assert_bump_round_trip("maven:com.example:demo", "1.2.3", "2.0.0", &after);
    assert_declared("maven:junit:junit", &before, "MODULE.bazel");
    assert!(matches!(
        add(
            "maven:com.example:demo",
            "1.2.3",
            &format!("{MAVEN_HEAD}maven.install(artifacts = ARTIFACTS)\n")
        ),
        Err(AddError::UnsupportedManifest { .. })
    ));
}

#[test]
fn cargo_inserts_into_dependencies_and_keeps_sorted_tables_sorted() {
    let before = "[package]\nname = \"hello\"\n\n[dependencies]\nanyhow = \"1\"\nzeta = \"1\" # pinned\n\n[lib]\nname = \"hello\"\n";
    let after = add("cargo:demo", "1.2.3", before).expect("insert");
    assert_eq!(
        after,
        "[package]\nname = \"hello\"\n\n[dependencies]\nanyhow = \"1\"\ndemo = \"1.2.3\"\nzeta = \"1\" # pinned\n\n[lib]\nname = \"hello\"\n"
    );
    assert_bump_round_trip("cargo:demo", "1.2.3", "2.0.0", &after);
    let unsorted = "[dependencies]\nzeta = \"1\"\nanyhow = \"1\"\n";
    assert_eq!(
        add("cargo:demo", "1.2.3", unsorted).as_deref(),
        Ok("[dependencies]\nzeta = \"1\"\nanyhow = \"1\"\ndemo = \"1.2.3\"\n")
    );
    let missing = add("cargo:demo", "1.2.3", "[package]\nname = \"hello\"\n").expect("insert");
    assert_eq!(
        missing,
        "[package]\nname = \"hello\"\n\n[dependencies]\ndemo = \"1.2.3\"\n"
    );
    assert_bump_round_trip("cargo:demo", "1.2.3", "2.0.0", &missing);
    assert_declared(
        "cargo:anyhow",
        before,
        "rust/tests/fixtures/hello/Cargo.toml",
    );
    assert_declared(
        "cargo:demo",
        "[dev-dependencies]\ndemo = \"1\"\n",
        "rust/tests/fixtures/hello/Cargo.toml",
    );
    assert_declared(
        "cargo:demo",
        "[target.'cfg(unix)'.dependencies]\ndemo = { version = \"1\" }\n",
        "rust/tests/fixtures/hello/Cargo.toml",
    );
    assert!(matches!(
        add("cargo:demo", "1.2.3", "[dependencies\n"),
        Err(AddError::UnsupportedManifest { .. })
    ));
    assert!(matches!(
        add("cargo:demo", "1.2.3", "dependencies = 1\n"),
        Err(AddError::UnsupportedManifest { .. })
    ));
}

#[test]
fn npm_inserts_into_dependencies_keeping_indentation() {
    let before = "{\n  \"name\": \"x\",\n  \"dependencies\": {\n    \"a\": \"1.0.0\"\n  },\n  \"devDependencies\": {\n    \"b\": \"1.0.0\"\n  }\n}\n";
    let after = add("npm:demo", "1.2.3", before).expect("insert");
    assert_eq!(
        after,
        "{\n  \"name\": \"x\",\n  \"dependencies\": {\n    \"a\": \"1.0.0\",\n    \"demo\": \"1.2.3\"\n  },\n  \"devDependencies\": {\n    \"b\": \"1.0.0\"\n  }\n}\n"
    );
    assert_bump_round_trip("npm:demo", "1.2.3", "2.0.0", &after);
    assert_declared("npm:b", before, "package.json");

    let missing = "{\r\n    \"name\": \"x\",\r\n    \"devDependencies\": {\r\n        \"b\": \"1.0.0\"\r\n    }\r\n}\r\n";
    let created = add("npm:@scope/demo", "1.2.3", missing).expect("insert");
    assert_eq!(
        created,
        "{\r\n    \"name\": \"x\",\r\n    \"dependencies\": {\r\n        \"@scope/demo\": \"1.2.3\"\r\n    },\r\n    \"devDependencies\": {\r\n        \"b\": \"1.0.0\"\r\n    }\r\n}\r\n"
    );
    assert_bump_round_trip("npm:@scope/demo", "1.2.3", "2.0.0", &created);

    let last = add("npm:demo", "1.2.3", "{\n  \"name\": \"x\"\n}\n").expect("insert");
    assert_eq!(
        last,
        "{\n  \"name\": \"x\",\n  \"dependencies\": {\n    \"demo\": \"1.2.3\"\n  }\n}\n"
    );
    let empty = add(
        "npm:demo",
        "1.2.3",
        "{\n  \"name\": \"x\",\n  \"dependencies\": {}\n}\n",
    )
    .expect("insert");
    assert_eq!(
        empty,
        "{\n  \"name\": \"x\",\n  \"dependencies\": {\n    \"demo\": \"1.2.3\"\n  }\n}\n"
    );
    let inline = add("npm:demo", "1.2.3", "{\"dependencies\": {\"a\": \"1\"}}").expect("insert");
    assert_eq!(
        inline,
        "{\"dependencies\": {\"a\": \"1\", \"demo\": \"1.2.3\"}}"
    );
    for section in ["peerDependencies", "optionalDependencies"] {
        assert_declared(
            "npm:demo",
            &format!("{{\"{section}\": {{\"demo\": \"1\"}}}}"),
            "package.json",
        );
    }
    for malformed in ["{", "[]", "{\"dependencies\": []}", "{}"] {
        assert!(
            matches!(
                add("npm:demo", "1.2.3", malformed),
                Err(AddError::UnsupportedManifest { .. })
            ),
            "{malformed}"
        );
    }
}

#[test]
fn go_inserts_a_direct_require() {
    let before = "module example.com/root\n\ngo 1.24\n\nrequire (\n\texample.com/a v1.0.0\n\texample.com/z v1.0.0\n)\n";
    let after = add("go:example.com/demo", "1.2.3", before).expect("insert");
    assert_eq!(
        after,
        "module example.com/root\n\ngo 1.24\n\nrequire (\n\texample.com/a v1.0.0\n\texample.com/demo v1.2.3\n\texample.com/z v1.0.0\n)\n"
    );
    assert_bump_round_trip("go:example.com/demo", "1.2.3", "2.0.0", &after);
    assert_declared("go:example.com/a", before, "third_party/go/go.mod");
    assert!(matches!(
        add("go:example.com/demo", "1.2.3", "module x\nrequire (\n"),
        Err(AddError::UnsupportedManifest { .. })
    ));
}

#[test]
fn nuget_inserts_after_the_last_main_group_line() {
    let before = "source https://api.nuget.org/v3/index.json\nframework: net10.0\n\nnuget FSharp.Core 10.1.201\nnuget xunit.v3 4.0.0\n\ngroup lint\nsource https://api.nuget.org/v3/index.json\n\nnuget Other 1.0.0\n";
    let after = add("nuget:Demo.Lib", "1.2.3", before).expect("insert");
    assert_eq!(
        after,
        "source https://api.nuget.org/v3/index.json\nframework: net10.0\n\nnuget FSharp.Core 10.1.201\nnuget xunit.v3 4.0.0\nnuget Demo.Lib 1.2.3\n\ngroup lint\nsource https://api.nuget.org/v3/index.json\n\nnuget Other 1.0.0\n"
    );
    assert_bump_round_trip("nuget:Demo.Lib", "1.2.3", "2.0.0", &after);
    assert_declared(
        "nuget:fsharp.core",
        before,
        "third_party/dotnet/paket.dependencies",
    );
    assert_declared(
        "nuget:Other",
        before,
        "third_party/dotnet/paket.dependencies",
    );
    let bare = add(
        "nuget:Demo.Lib",
        "1.2.3",
        "source https://api.nuget.org/v3/index.json\r\n",
    )
    .expect("insert");
    assert_eq!(
        bare,
        "source https://api.nuget.org/v3/index.json\r\n\r\nnuget Demo.Lib 1.2.3\r\n"
    );
    assert!(matches!(
        add("nuget:Demo.Lib", "1.2.3", "# only a comment\n"),
        Err(AddError::UnsupportedManifest { .. })
    ));
    assert!(matches!(
        add("nuget:Demo.Lib", "1.2.3", "nuget Demo.Lib >= ~ 1\n"),
        Err(AddError::UnsupportedManifest { .. })
    ));
}
