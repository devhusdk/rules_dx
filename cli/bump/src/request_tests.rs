use super::*;

const MAVEN_HEAD: &str =
    "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n";

fn maven_module(artifacts: &str) -> String {
    format!("{MAVEN_HEAD}maven.install(\n    artifacts = [\n{artifacts}    ],\n)\n")
}

#[test]
fn cargo_table_variants_preserve_comments_and_reject_nonregistry_shapes() {
    let bump = BumpRequest::parse("cargo:demo", "2.0.0").expect("request");
    for table in [
        "workspace.dependencies",
        "target.'cfg(unix)'.dependencies",
        "target.'cfg(unix)'.dev-dependencies",
        "target.'cfg(unix)'.build-dependencies",
        "patch.crates-io",
    ] {
        let source = format!("[{table}]\ndemo = \"1.0.0\" # pinned\n");
        assert_eq!(
            bump.plan_edit(&source).expect("edit"),
            source.replace("1.0.0", "2.0.0")
        );
    }
    for body in [
        "git = 'https://example.com/demo'",
        "path = '../demo'",
        "workspace = true",
        "version = 1",
        "features = []",
    ] {
        let source = format!("[dependencies.demo]\n{body}\n");
        assert!(
            matches!(
                bump.plan_edit(&source),
                Err(BumpError::UnsupportedManifest { .. })
            ),
            "{body}"
        );
    }
    for source in ["[dependencies]\ndemo = 1\n", "[dependencies]\ndemo = {}\n"] {
        assert!(matches!(
            bump.plan_edit(source),
            Err(BumpError::UnsupportedManifest { .. })
        ));
    }
    let tag = WidenVersion::GitTag("v2".to_owned());
    assert!(matches!(
        plan_cargo_toml("", "demo", &tag),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    let mut doc: toml_edit::DocumentMut =
        "[dependencies]\nnumber = 1\ninline = {}\n[dependencies.table]\nfeatures = []\n"
            .parse()
            .expect("doc");
    let table = doc["dependencies"].as_table_mut().expect("deps");
    for package in ["missing", "number", "inline", "table"] {
        assert!(!cargo_set_version(table, package, "2.0.0"));
    }
}
#[test]
fn hand_built_requests_reject_wrong_version_kinds() {
    let tag = WidenVersion::GitTag("v2".to_owned());
    for set in [
        BumpSet::Bazel,
        BumpSet::Npm,
        BumpSet::Go,
        BumpSet::Maven,
        BumpSet::NuGet,
    ] {
        let request = BumpRequest {
            set,
            package: "demo".to_owned(),
            version: tag.clone(),
            selector: "demo".to_owned(),
            raw_version: "v2".to_owned(),
            manifest: None,
        };
        assert!(matches!(
            request.plan_edit(""),
            Err(BumpError::UnsupportedManifest { .. })
        ));
    }
    assert!(matches!(
        plan_bazelversion("1.0.0", &tag),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    let semver = WidenVersion::Semver("2.0.0".parse().expect("version"));
    assert!(matches!(
        plan_github_workflow("", "actions/checkout", &semver),
        Err(BumpError::UnsupportedManifest { .. })
    ));
}

#[test]
fn package_validation_rejects_invalid_ecosystem_identities() {
    for (set, packages) in [
        (BumpSet::Bazel, vec!["bad!", ""]),
        (
            BumpSet::Npm,
            vec![
                "",
                "a:b",
                "a b",
                "@scope",
                "@/name",
                "@scope/",
                "@scope/bad!",
                "a/b",
                "bad!",
            ],
        ),
        (
            BumpSet::Go,
            vec!["", "a:b", "a b", "/a", "a/", "a//b", "a!b"],
        ),
        (
            BumpSet::GithubActions,
            vec![
                "owner",
                "/repo",
                "owner/",
                "owner/a/b",
                "owner/a b",
                "a b/repo",
                "bad!/repo",
                "owner/bad!",
            ],
        ),
        (BumpSet::Maven, vec!["bad!:artifact", "group:bad!"]),
        (BumpSet::NuGet, vec!["bad!", ""]),
    ] {
        for package in packages {
            assert!(
                matches!(
                    validate_package(set, package),
                    Err(BumpError::InvalidPackage { .. })
                ),
                "{}:{package}",
                set.name()
            );
        }
    }
    assert!(is_target_shape("unknown:a/b"));
    assert!(is_target_shape("//x"));
    assert!(is_target_shape("@r//:t"));
    assert!(!is_target_shape("cargo:anyhow"));
}

#[test]
fn widen_rejects_missing_ambiguous_and_malformed_requirements() {
    for (selector, missing, duplicate, malformed) in [
        (
            "bazel:demo",
            "module(name = \"root\")\n",
            "bazel_dep(name = \"demo\", version = \"1.0.0\")\nbazel_dep(name = \"demo\", version = \"1.1.0\")\n",
            "bazel_dep(name = \"demo\")\n",
        ),
        (
            "npm:demo",
            "{\"other\": \"1.0.0\"}\n",
            "{\n\"dependencies\": {\n\"demo\": \"1.0.0\"\n},\n\"devDependencies\": {\n\"demo\": \"1.1.0\"\n}\n}\n",
            "{\n\"dependencies\": {\n\"demo\": 1\n}\n}\n",
        ),
        (
            "go:example.com/demo",
            "module example.com/root\n",
            "module example.com/root\nrequire example.com/demo v1.0.0\nrequire example.com/demo v1.1.0\n",
            "module example.com/root\nrequire example.com/demo\n",
        ),
        (
            "maven:org.example:demo",
            "maven.install(artifacts = [])\n",
            "SKIP",
            "SKIP",
        ),
        (
            "nuget:Demo",
            "source https://example.com\n",
            "nuget Demo 1.0.0\nnuget Demo 1.1.0\n",
            "nuget Demo\n",
        ),
    ] {
        let bump = BumpRequest::parse(selector, "2.0.0").expect("request");
        let (duplicate, malformed) = if duplicate == "SKIP" {
            (
                maven_module("\"org.example:demo:1.0.0\",\n\"org.example:demo:1.1.0\",\n"),
                maven_module("\"org.example:demo:\",\n"),
            )
        } else {
            (duplicate.to_owned(), malformed.to_owned())
        };
        let (duplicate, malformed) = (duplicate.as_str(), malformed.as_str());
        assert!(matches!(bump.plan_edit(missing), Err(BumpError::NotFound { .. })), "{selector}");
        assert!(matches!(bump.plan_edit(duplicate), Err(BumpError::Ambiguous { count: 2, .. })), "{selector}");
        assert!(matches!(bump.plan_edit(malformed), Err(BumpError::UnsupportedManifest { .. })), "{selector}");
    }
    let bump = BumpRequest::parse("npm:demo", "2.0.0").expect("npm");
    assert!(matches!(
        bump.plan_edit("not json"),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    assert!(matches!(
        bump.plan_edit("{\"other\": \"demo\"}"),
        Err(BumpError::NotFound { .. })
    ));
    let bump = BumpRequest::parse("bazel:.bazelversion", "2.0.0").expect("bazel");
    assert!(matches!(
        bump.plan_edit(" \n"),
        Err(BumpError::UnsupportedManifest { .. })
    ));
}

#[test]
fn widen_preserves_comments_crlf_and_package_boundaries() {
    for (selector, input, expected) in [
        ("go:example.com/demo", "module example.com/root\n// example.com/demo v9.0.0\nrequire (\n example.com/demo-extra v1.0.0\n example.com/demo v1.2.3-beta+build // indirect\n)\n", "module example.com/root\n// example.com/demo v9.0.0\nrequire (\n example.com/demo-extra v1.0.0\n example.com/demo v2.0.0 // indirect\n)\n"),
        ("nuget:Demo", "# nuget Demo 9.0.0\r\n// keep\r\nnuget Demo.Other 1.0.0\r\n  nuget Demo 1.2.3  # pinned\r\n", "# nuget Demo 9.0.0\r\n// keep\r\nnuget Demo.Other 1.0.0\r\n  nuget Demo 2.0.0  # pinned\r\n"),
        ("nuget:Demo", "nuget Demo 1.2.3", "nuget Demo 2.0.0"),
    ] {
        let bump = BumpRequest::parse(selector, "2.0.0").expect("request");
        assert_eq!(bump.plan_edit(input).expect("edit"), expected);
    }
}

fn steps(lines: &str) -> String {
    format!("jobs:\n  b:\n    steps:\n{lines}")
}

fn widen(source: &str) -> Result<String, BumpError> {
    BumpRequest::parse("github-actions:actions/checkout", &"a".repeat(40))
        .expect("request")
        .plan_edit(source)
}

#[test]
fn workflow_widen_requires_one_nonempty_pin() {
    let sha = "a".repeat(40);
    let bump = BumpRequest::parse("github-actions:actions/checkout", &sha).expect("request");
    assert_eq!(bump.target_manifest(), ".github/workflows");
    assert!(matches!(
        bump.plan_edit("steps: []\n"),
        Err(BumpError::NotFound { manifest, .. }) if manifest == ".github/workflows"
    ));
    assert!(matches!(
        bump.plan_edit(&steps("      - uses: actions/checkout@\n")),
        Err(BumpError::UnsupportedManifest { manifest, .. }) if manifest == ".github/workflows"
    ));
    assert!(matches!(
        bump.plan_edit(&steps(
            "      - uses: actions/checkout@v1\n      - uses: actions/checkout@v2\n"
        )),
        Err(BumpError::Ambiguous { count: 2, manifest, .. }) if manifest == ".github/workflows"
    ));
    assert_eq!(
        bump.plan_edit(&steps("      - uses: actions/checkout@v1"))
            .expect("edit"),
        steps(&format!("      - uses: actions/checkout@{sha}"))
    );
}

#[test]
fn workflow_widen_keeps_quotes_comments_and_formatting() {
    let sha = "a".repeat(40);
    for (before, after) in [
        (
            "      - uses: \"actions/checkout@OLD\" # pinned\n",
            format!("      - uses: \"actions/checkout@{sha}\" # pinned\n"),
        ),
        (
            "      - uses: 'actions/checkout@OLD'   # v4\n",
            format!("      - uses: 'actions/checkout@{sha}'   # v4\n"),
        ),
        (
            "      - uses: actions/checkout@v4 # v4\n",
            format!("      - uses: actions/checkout@{sha} # v4\n"),
        ),
        (
            "      - {name: x, uses: actions/checkout@v4, with: {a: 1}}\n",
            format!("      - {{name: x, uses: actions/checkout@{sha}, with: {{a: 1}}}}\n"),
        ),
        (
            "      - name: x\n        uses: \"actions/checkout@v4\"\n        with:\n          a: 1\n",
            format!("      - name: x\n        uses: \"actions/checkout@{sha}\"\n        with:\n          a: 1\n"),
        ),
        (
            "      - uses: \"actions\\x2fcheckout@v4\"\n",
            format!("      - uses: \"actions\\x2fcheckout@{sha}\"\n"),
        ),
        (
            "      - uses: actions/checkout/sub@v4\n",
            format!("      - uses: actions/checkout/sub@{sha}\n"),
        ),
        (
            "      - uses: Actions/Checkout@v4\n",
            format!("      - uses: Actions/Checkout@{sha}\n"),
        ),
    ] {
        let source = steps(before);
        let widened = widen(&source).expect(before);
        assert_eq!(widened, steps(&after));
        assert_eq!(widen(&widened).expect("second run"), widened);
    }
    let crlf =
        steps("      - uses: actions/checkout@v4\r\n      - run: x\r\n").replace("\n  ", "\r\n  ");
    let widened = widen(&crlf).expect("crlf");
    assert_eq!(widened, crlf.replace("@v4", &format!("@{sha}")));
}

#[test]
fn workflow_widen_selects_job_level_reusable_workflows() {
    let sha = "a".repeat(40);
    let bump = BumpRequest::parse("github-actions:org/repo", &sha).expect("request");
    let source = "jobs:\n  call:\n    uses: org/repo/.github/workflows/ci.yml@v1 # v1\n    with:\n      a: 1\n";
    assert_eq!(
        bump.plan_edit(source).expect("edit"),
        format!("jobs:\n  call:\n    uses: org/repo/.github/workflows/ci.yml@{sha} # v1\n    with:\n      a: 1\n")
    );
    let not_workflow = "jobs:\n  call:\n    uses: org/repo/other/ci.yml@v1\n";
    assert!(matches!(
        bump.plan_edit(not_workflow),
        Err(BumpError::NotFound { .. })
    ));
    let bare = "jobs:\n  call:\n    uses: org/repo@v1\n";
    assert!(matches!(
        bump.plan_edit(bare),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn workflow_widen_ignores_text_that_is_not_a_real_pin() {
    for source in [
        "# - uses: actions/checkout@v1\njobs:\n  b:\n    steps:\n      - run: echo hi\n",
        "jobs:\n  b:\n    steps:\n      - run: |\n          uses: actions/checkout@v1\n          - uses: actions/checkout@v1\n",
        "jobs:\n  b:\n    steps:\n      - run: >\n          uses: actions/checkout@v1\n",
        "jobs:\n  b:\n    steps:\n      - name: uses actions/checkout@v1\n        env:\n          uses: actions/checkout@v1\n",
        "jobs:\n  b:\n    steps:\n      - uses: actions/cache@v1\n      - uses: actions/checkout-extra@v1\n      - uses: my-actions/checkout@v1\n      - uses: actions/checkoutx/sub@v1\n",
        "jobs:\n  b:\n    steps:\n      - uses: ./actions/checkout@v1\n      - uses: docker://actions/checkout@v1\n      - uses: ${{ matrix.action }}@v1\n",
        "steps:\n  - uses: actions/checkout@v1\n",
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout\n",
    ] {
        assert!(
            matches!(widen(source), Err(BumpError::NotFound { .. })),
            "{source}"
        );
    }
}

#[test]
fn workflow_widen_only_edits_the_real_pin_beside_fake_text() {
    let sha = "a".repeat(40);
    let source = steps("      - run: |\n          uses: actions/checkout@v1\n      # uses: actions/checkout@v1\n      - uses: actions/checkout@v1 # uses: actions/checkout@v1\n");
    let widened = widen(&source).expect("edit");
    assert_eq!(
        widened,
        steps(&format!("      - run: |\n          uses: actions/checkout@v1\n      # uses: actions/checkout@v1\n      - uses: actions/checkout@{sha} # uses: actions/checkout@v1\n"))
    );
}

#[test]
fn workflow_widen_reports_unsupported_yaml_without_edits() {
    for source in [
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@v1\n  bad: [\n",
        "jobs:\n  b: {\n",
        "jobs:\n  b:\n    steps:\n      - uses: \"actions/checkout@v1\n",
        "x: &s\n  - uses: actions/checkout@v1\njobs:\n  b:\n    steps: *s\n",
        "base: &base\n  uses: actions/checkout@v1\njobs:\n  b:\n    steps:\n      - <<: *base\n",
        "jobs:\n  b:\n    steps:\n      - uses: |\n          actions/checkout@v1\n",
        "jobs:\n  b:\n    steps:\n      - uses: \"actions/checkout@v\\x31\"\n",
    ] {
        assert!(
            matches!(widen(source), Err(BumpError::UnsupportedManifest { .. })),
            "{source}"
        );
    }
    let aliased_elsewhere =
        "env: &e\n  A: 1\njobs:\n  b:\n    env: *e\n    steps:\n      - uses: actions/cache@v1\n";
    assert!(matches!(
        widen(aliased_elsewhere),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn only_yaml_and_yml_are_workflow_files() {
    assert!(crate::gha::workflow_file("ci.yml"));
    assert!(crate::gha::workflow_file("reusable.yaml"));
    assert!(!crate::gha::workflow_file("README.md"));
    assert!(!crate::gha::workflow_file("ci.yml.bak"));
    assert!(!crate::gha::workflow_file("yml"));
}

#[test]
fn parses_single_requirement_shapes() {
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    assert_eq!(bump.set, BumpSet::Cargo);
    assert_eq!(bump.package, "anyhow");
    assert_eq!(
        bump.target_manifest(),
        "rust/tests/fixtures/hello/Cargo.toml"
    );
    assert!(bump.needs_update_refresh());

    let bump = BumpRequest::parse("npm:react", "1.2.3").expect("npm");
    assert_eq!(bump.target_manifest(), "package.json");

    let bump = BumpRequest::parse("npm:@astrojs/compiler", "1.2.3").expect("scoped");
    assert_eq!(bump.package, "@astrojs/compiler");

    let bump = BumpRequest::parse("bazel:rules_rust", "0.74.0").expect("bazel module");
    assert_eq!(bump.target_manifest(), "MODULE.bazel");
    assert!(!bump.needs_update_refresh());

    let bump = BumpRequest::parse("bazel:.bazelversion", "9.2.0").expect("bazelversion");
    assert_eq!(bump.target_manifest(), ".bazelversion");

    let bump = BumpRequest::parse("github-actions:actions/checkout", "v4").expect("gha");
    assert_eq!(bump.set, BumpSet::GithubActions);
    assert!(!bump.needs_update_refresh());

    let bump = BumpRequest::parse("go:example.com/mod", "1.2.3").expect("go");
    assert_eq!(bump.target_manifest(), "third_party/go/go.mod");

    let bump = BumpRequest::parse("maven:junit:junit", "4.13.2").expect("maven");
    assert_eq!(bump.set, BumpSet::Maven);
    assert_eq!(bump.package, "junit:junit");
    assert_eq!(bump.target_manifest(), "MODULE.bazel");
    assert!(bump.needs_update_refresh());

    let bump = BumpRequest::parse("maven:org.junit.jupiter:junit-jupiter-api", "6.1.3")
        .expect("maven jupiter");
    assert_eq!(bump.package, "org.junit.jupiter:junit-jupiter-api");

    let bump = BumpRequest::parse("nuget:FSharp.Core", "10.1.201").expect("nuget");
    assert_eq!(bump.set, BumpSet::NuGet);
    assert_eq!(
        bump.target_manifest(),
        "third_party/dotnet/paket.dependencies"
    );
    assert!(bump.needs_update_refresh());
}

#[test]
fn removed_aliases_fail_closed() {
    assert!(BumpRequest::parse("gomod:example.com/mod", "1.2.3").is_err());
    assert!(BumpRequest::parse("gha:actions/checkout", "v4").is_err());
}

#[test]
fn bare_sets_targets_and_unknown_fail_closed() {
    assert!(matches!(
        BumpRequest::parse("cargo", "1.2.3"),
        Err(BumpError::BareSet { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("//rust/tests/fixtures/hello:hello", "1.2.3"),
        Err(BumpError::NotAPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("rust/tests/fixtures/hello/Cargo.toml", "1.2.3"),
        Err(BumpError::NotAPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("crates", "1.2.3"),
        Err(BumpError::UnknownSelector { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("cargo:", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("", "1.2.3"),
        Err(BumpError::Empty)
    ));
    assert!(matches!(
        BumpRequest::parse("cargo:anyhow", ""),
        Err(BumpError::Empty)
    ));
    assert!(matches!(
        BumpRequest::parse("cargo:bad name", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("maven", "1.2.3"),
        Err(BumpError::BareSet { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("nuget", "10.1.201"),
        Err(BumpError::BareSet { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("maven:junit", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("maven::artifact", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("maven:junit:junit:extra", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("nuget:", "10.1.201"),
        Err(BumpError::InvalidPackage { .. })
    ));
    assert!(matches!(
        BumpRequest::parse("cargo:anyhow:extra", "1.2.3"),
        Err(BumpError::InvalidPackage { .. })
    ));
}

#[test]
fn invalid_versions_fail_without_widen() {
    assert!(matches!(
        BumpRequest::parse("cargo:anyhow", "not-a-version!!!"),
        Err(BumpError::Version(_))
    ));
    assert!(matches!(
        BumpRequest::parse("github-actions:actions/checkout", "bad tag"),
        Err(BumpError::Version(_))
    ));
    assert!(matches!(
        BumpRequest::parse("maven:junit:junit", "not-a-version!!!"),
        Err(BumpError::Version(_))
    ));
    assert!(matches!(
        BumpRequest::parse("nuget:FSharp.Core", "not-a-version!!!"),
        Err(BumpError::Version(_))
    ));
}

#[test]
fn summary_names_selector_version_and_next_step() {
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    let summary = bump.summary();
    assert!(summary.contains("cargo:anyhow"), "{summary}");
    assert!(summary.contains("1.2.3"), "{summary}");
    assert!(summary.contains("dx update cargo"), "{summary}");
    assert!(summary.contains("automatically"), "{summary}");
    let bump = BumpRequest::parse("bazel:rules_rust", "0.74.0").expect("bazel");
    assert!(bump.summary().contains("flag-diff"), "{summary}");
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.3").expect("maven");
    assert!(bump.summary().contains("dx update maven"), "{summary}");
    assert!(bump.summary().contains("automatically"), "{summary}");
    let bump = BumpRequest::parse("nuget:FSharp.Core", "10.1.202").expect("nuget");
    assert!(bump.summary().contains("dx update nuget"), "{summary}");
}

#[test]
fn summary_carries_major_bump_migrate_hint_for_semver() {
    let bump = BumpRequest::parse("cargo:anyhow", "2.0.0").expect("cargo major");
    let summary = bump.summary();
    assert!(summary.contains("major bump"), "{summary}");
    assert!(summary.contains("dx migrate --from"), "{summary}");
    assert!(summary.contains("migrate_failed"), "{summary}");
    assert!(summary.contains("missing-versions"), "{summary}");
    let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
    let bump = BumpRequest::parse("github-actions:actions/checkout", sha).expect("gha");
    assert!(!bump.summary().contains("major bump"), "{}", bump.summary());
}

#[test]
fn refresh_selector_chains_automatically_per_set() {
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    assert_eq!(bump.refresh_selector(), "cargo");
    assert!(bump.needs_update_refresh());
    let bump = BumpRequest::parse("npm:jest", "30.3.0").expect("npm");
    assert_eq!(bump.refresh_selector(), "npm:jest");
    assert!(bump.needs_update_refresh());
    let bump = BumpRequest::parse("npm:@astrojs/compiler", "1.2.3").expect("scoped");
    assert_eq!(bump.refresh_selector(), "npm:@astrojs/compiler");
    let bump = BumpRequest::parse("go:example.com/mod", "1.2.3").expect("go");
    assert_eq!(bump.refresh_selector(), "go");
    assert!(bump.needs_update_refresh());
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.2").expect("maven");
    assert_eq!(bump.refresh_selector(), "maven");
    assert!(bump.needs_update_refresh());
    let bump = BumpRequest::parse("nuget:FSharp.Core", "10.1.201").expect("nuget");
    assert_eq!(bump.refresh_selector(), "nuget");
    assert!(bump.needs_update_refresh());
    let bump = BumpRequest::parse("bazel:rules_rust", "0.74.0").expect("bazel");
    assert_eq!(bump.refresh_selector(), "");
    assert!(!bump.needs_update_refresh());
    let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
    let bump = BumpRequest::parse("github-actions:actions/checkout", sha).expect("gha");
    assert_eq!(bump.refresh_selector(), "");
    assert!(!bump.needs_update_refresh());
}

#[test]
fn refresh_selector_is_the_set_name_plus_the_package_for_npm_only() {
    let cases = [
        ("cargo:anyhow", "1.2.3"),
        ("npm:jest", "30.3.0"),
        ("go:example.com/mod", "1.2.3"),
        ("maven:junit:junit", "4.13.2"),
        ("nuget:FSharp.Core", "10.1.201"),
        ("bazel:rules_rust", "0.74.0"),
        (
            "github-actions:actions/checkout",
            "3d3d42e5aac5ba805825da76410c181273ba90b1",
        ),
    ];
    assert_eq!(cases.len(), BumpSet::ALL.len(), "one case per set");
    for (selector, version) in cases {
        let bump = BumpRequest::parse(selector, version).expect("parse");
        let name = bump.set.name();
        let expected = if bump.set == BumpSet::Npm {
            format!("{name}:{}", bump.package)
        } else if bump.needs_update_refresh() {
            name.to_owned()
        } else {
            String::new()
        };
        assert_eq!(bump.refresh_selector(), expected, "{selector}");
    }
}

#[test]
fn edits_rewrite_exactly_one_requirement() {
    let bump = BumpRequest::parse("bazel:.bazelversion", "9.3.0").expect("bazelversion");
    assert_eq!(bump.plan_edit("9.2.0\n").expect("edit"), "9.3.0\n");
    assert_eq!(bump.plan_edit("9.2.0").expect("edit"), "9.3.0");
    assert!(matches!(
        bump.plan_edit("9.2.0\n9.3.0\n"),
        Err(BumpError::UnsupportedManifest { .. })
    ));

    let module = "bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n\
                  bazel_dep(name = \"gazelle\", version = \"0.52.2\")\n";
    let bump = BumpRequest::parse("bazel:rules_rust", "0.75.0").expect("module");
    let widened = bump.plan_edit(module).expect("edit");
    assert!(
        widened.contains("name = \"rules_rust\", version = \"0.75.0\""),
        "{widened}"
    );
    assert!(
        widened.contains("name = \"gazelle\", version = \"0.52.2\""),
        "{widened}"
    );
    assert!(matches!(
        bump.plan_edit("bazel_dep(name = \"other\", version = \"1.0.0\")\n"),
        Err(BumpError::NotFound { .. })
    ));

    let cargo = "[dependencies]\nanyhow = \"1\"\nserde = \"1\"\n";
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    let widened = bump.plan_edit(cargo).expect("edit");
    assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    assert!(widened.contains("serde = \"1\""), "{widened}");

    let npm =
        "{\n  \"devDependencies\": {\n    \"jest\": \"30.2.0\",\n    \"vue\": \"3.5.42\"\n  }\n}\n";
    let bump = BumpRequest::parse("npm:jest", "30.3.0").expect("npm");
    let widened = bump.plan_edit(npm).expect("edit");
    assert!(widened.contains("\"jest\": \"30.3.0\""), "{widened}");
    assert!(widened.contains("\"vue\": \"3.5.42\""), "{widened}");
    assert!(matches!(
        bump.plan_edit("{\n}\n"),
        Err(BumpError::NotFound { .. })
    ));

    let gomod = "module example.com/root\n\nrequire example.com/mod v1.2.3\n";
    let bump = BumpRequest::parse("go:example.com/mod", "1.3.0").expect("go");
    let widened = bump.plan_edit(gomod).expect("edit");
    assert!(widened.contains("example.com/mod v1.3.0"), "{widened}");

    let module = maven_module(
        "        \"junit:junit:4.13.2\",\n        \"org.junit.jupiter:junit-jupiter-api:6.1.3\",\n",
    );
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.3").expect("maven");
    let widened = bump.plan_edit(&module).expect("edit");
    assert!(widened.contains("\"junit:junit:4.13.3\""), "{widened}");
    assert!(
        widened.contains("\"org.junit.jupiter:junit-jupiter-api:6.1.3\""),
        "{widened}"
    );
    assert!(matches!(
        bump.plan_edit(&maven_module("")),
        Err(BumpError::NotFound { .. })
    ));

    let paket = "source https://api.nuget.org/v3/index.json\nframework: net10.0\n\nnuget FSharp.Core 10.1.201\nnuget xunit.v3 4.0.0\n";
    let bump = BumpRequest::parse("nuget:FSharp.Core", "10.1.202").expect("nuget");
    let widened = bump.plan_edit(paket).expect("edit");
    assert!(widened.contains("nuget FSharp.Core 10.1.202"), "{widened}");
    assert!(widened.contains("nuget xunit.v3 4.0.0"), "{widened}");
    assert!(matches!(
        bump.plan_edit("source https://api.nuget.org/v3/index.json\n"),
        Err(BumpError::NotFound { .. })
    ));
}

fn paket_edit(line: &str) -> Result<String, BumpError> {
    let bump = BumpRequest::parse("nuget:Newtonsoft.Json", "13.0.3").expect("nuget");
    bump.plan_edit(&format!(
        "source https://api.nuget.org/v3/index.json\n{line}\nnuget Other.Pkg >= 1.0.0\n"
    ))
}

#[test]
fn paket_requirements_are_replaced_as_structured_syntax() {
    let cases = [
        (
            "nuget Newtonsoft.Json 12.0.0",
            "nuget Newtonsoft.Json 13.0.3",
        ),
        (
            "nuget Newtonsoft.Json >= 12.0.0",
            "nuget Newtonsoft.Json 13.0.3",
        ),
        (
            "nuget Newtonsoft.Json >=12.0.0",
            "nuget Newtonsoft.Json 13.0.3",
        ),
        (
            "nuget Newtonsoft.Json ~> 12.0",
            "nuget Newtonsoft.Json 13.0.3",
        ),
        (
            "nuget Newtonsoft.Json >= 12.0.0 < 14.0.0",
            "nuget Newtonsoft.Json 13.0.3",
        ),
        (
            "nuget Newtonsoft.Json >= 12.0.0 prerelease",
            "nuget Newtonsoft.Json 13.0.3 prerelease",
        ),
        (
            "nuget Newtonsoft.Json ~> 12.0 storage: none, copy_local: false # keep 12.0",
            "nuget Newtonsoft.Json 13.0.3 storage: none, copy_local: false # keep 12.0",
        ),
        (
            "  NuGet newtonsoft.json >= 12.0.0 # keep",
            "  NuGet newtonsoft.json 13.0.3 # keep",
        ),
    ];
    for (before, after) in cases {
        let edited = paket_edit(before).unwrap_or_else(|err| panic!("{before}: {err:?}"));
        let expected = format!(
            "source https://api.nuget.org/v3/index.json\n{after}\nnuget Other.Pkg >= 1.0.0\n"
        );
        assert_eq!(edited, expected, "{before}");
    }
}

#[test]
fn paket_edits_touch_only_the_selected_group_declaration() {
    let bump = BumpRequest::parse("nuget:Newtonsoft.Json", "13.0.3").expect("nuget");
    let paket =
        "group Build\n  nuget Newtonsoft.Json >= 12.0.0\n\ngroup Main\n  nuget Other.Pkg ~> 1.0\n";
    assert_eq!(
        bump.plan_edit(paket).expect("edit"),
        "group Build\n  nuget Newtonsoft.Json 13.0.3\n\ngroup Main\n  nuget Other.Pkg ~> 1.0\n"
    );
    let paket = "nuget Newtonsoft.Json >= 12.0.0\ngroup Build\n  nuget Newtonsoft.Json ~> 12.0\n";
    assert!(matches!(
        bump.plan_edit(paket),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
}

#[test]
fn paket_malformed_requirements_fail_without_edits() {
    for line in [
        "nuget Newtonsoft.Json",
        "nuget Newtonsoft.Json >=",
        "nuget Newtonsoft.Json >= >= 1.0",
        "nuget Newtonsoft.Json >= 12.0.0 < ",
        "nuget Newtonsoft.Json storage: none",
        "nuget Newtonsoft.Json >= 12.0.0 || 13.0",
        "nuget Newtonsoft.Json >=1.0$",
    ] {
        assert!(
            matches!(paket_edit(line), Err(BumpError::UnsupportedManifest { .. })),
            "{line}"
        );
    }
}

#[test]
fn maven_and_nuget_edits_fail_closed_on_ambiguous_and_prefixes() {
    let module = maven_module("        \"junit:junit:4.13.2\",\n        \"junit:junit:4.13.2\",\n");
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.3").expect("maven");
    assert!(matches!(
        bump.plan_edit(&module),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
    let module = maven_module("        \"junit:junit-jupiter:1.0.0\",\n");
    assert!(matches!(
        bump.plan_edit(&module),
        Err(BumpError::NotFound { .. })
    ));
    let paket = "nuget FSharp.Core 10.1.201\nnuget FSharp.Core 10.1.201\n";
    let bump = BumpRequest::parse("nuget:FSharp.Core", "10.1.202").expect("nuget");
    assert!(matches!(
        bump.plan_edit(paket),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
    let paket = "nuget xunit.v3.assert 4.0.0\n";
    let bump = BumpRequest::parse("nuget:xunit.v3", "4.0.1").expect("nuget prefix");
    assert!(matches!(
        bump.plan_edit(paket),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn cargo_toml_preserves_format_and_fails_closed() {
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");

    let cargo = "[dependencies]\nanyhow = { version = \"1\", features = [\"derive\"] } # keep\nserde = \"1\"\n";
    let widened = bump.plan_edit(cargo).expect("inline");
    assert!(
        widened.contains("anyhow = { version = \"1.2.3\""),
        "{widened}"
    );
    assert!(widened.contains("features = [\"derive\"]"), "{widened}");
    assert!(widened.contains("# keep"), "{widened}");
    assert!(widened.contains("serde = \"1\""), "{widened}");

    let cargo = "[dependencies.anyhow]\nversion = \"1\"\nfeatures = [\"derive\"]\n";
    let widened = bump.plan_edit(cargo).expect("table");
    assert!(widened.contains("version = \"1.2.3\""), "{widened}");
    assert!(widened.contains("features ="), "{widened}");

    let cargo = "[dev-dependencies]\nanyhow = \"1\"\n";
    let widened = bump.plan_edit(cargo).expect("dev");
    assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");

    let cargo = "[dependencies]\nanyhow = \"1\"\n[dev-dependencies]\nanyhow = \"1\"\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));

    let cargo = "[dependencies]\nanyhow = { git = \"https://example.com/repo\", tag = \"v1\" }\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    let cargo = "[dependencies]\nanyhow = { path = \"../anyhow\" }\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::UnsupportedManifest { .. })
    ));

    let cargo = "[dependencies]\nanyhow = { workspace = true }\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    let cargo = "[dependencies]\nanyhow = { optional = true }\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::UnsupportedManifest { .. })
    ));

    assert!(matches!(
        bump.plan_edit("[dependencies\nanyhow = "),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    assert!(matches!(
        bump.plan_edit("[dependencies]\nserde = \"1\"\n"),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn github_actions_needs_sha_for_tags() {
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let workflow = steps(&format!("      - uses: actions/checkout@{old} # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n", old = "3d3c42e5aac5ba805825da76410c181273ba90b1"));
    let bump = BumpRequest::parse("github-actions:actions/checkout", sha).expect("sha");
    let widened = bump.plan_edit(&workflow).expect("edit");
    assert!(
        widened.contains(&format!("actions/checkout@{sha}")),
        "{widened}"
    );
    assert!(widened.contains("actions/cache@bbbb"), "{widened}");

    let tag = BumpRequest::parse("github-actions:actions/checkout", "v5").expect("tag");
    assert!(matches!(
        tag.plan_edit(&workflow),
        Err(BumpError::NeedsSha { .. })
    ));
}

#[test]
fn cargo_boundary_prefers_exact_table_key() {
    let bump = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    let cargo = "[dependencies]\nmy-anyhow = \"1\"\n";
    assert!(matches!(
        bump.plan_edit(cargo),
        Err(BumpError::NotFound { .. })
    ));
    let cargo = "[dependencies]\nmy-anyhow = \"1\"\nanyhow = \"1\"\n";
    let widened = bump.plan_edit(cargo).expect("exact");
    assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    assert!(widened.contains("my-anyhow = \"1\""), "{widened}");
}

#[test]
fn module_bazel_edits_one_literal_version_token() {
    let bump = BumpRequest::parse("bazel:rules_rust", "0.75.0").expect("module");
    for (before, after) in [
        (
            "bazel_dep(name = \"rules_rust\",version=\"0.74.0\")\n",
            "bazel_dep(name = \"rules_rust\",version=\"0.75.0\")\n",
        ),
        (
            "bazel_dep(name = \"rules_rust\", version   =   \"0.74.0\")\n",
            "bazel_dep(name = \"rules_rust\", version   =   \"0.75.0\")\n",
        ),
        (
            "bazel_dep(\n    version = '0.74.0',  # (pinned)\n    name = 'rules_rust',\n)\n",
            "bazel_dep(\n    version = '0.75.0',  # (pinned)\n    name = 'rules_rust',\n)\n",
        ),
        (
            "# bazel_dep(name = \"rules_rust\", version = \"0.1.0\")\nX = \"bazel_dep(name = 'rules_rust', version = '0.2.0')\"\nbazel_dep(name = \"rules_rust\", version = \"\"\"0.74.0\"\"\", dev_dependency = [1, (2)] != [])\n",
            "# bazel_dep(name = \"rules_rust\", version = \"0.1.0\")\nX = \"bazel_dep(name = 'rules_rust', version = '0.2.0')\"\nbazel_dep(name = \"rules_rust\", version = \"\"\"0.75.0\"\"\", dev_dependency = [1, (2)] != [])\n",
        ),
    ] {
        assert_eq!(
            bump.plan_edit_outcome(before).expect(before),
            BumpPlanOutcome::Changed(after.to_owned())
        );
        assert_eq!(
            bump.plan_edit_outcome(after).expect(after),
            BumpPlanOutcome::Unchanged
        );
    }
    for (source, reason) in [
        (
            "bazel_dep(name = \"rules_rust\", version = V)\n",
            "bazel_dep \"rules_rust\" at line 1 has a computed version",
        ),
        (
            "bazel_dep(name = \"rules_rust\")\n",
            "bazel_dep \"rules_rust\" at line 1 has no version",
        ),
        (
            "bazel_dep(name = NAME, version = \"0.74.0\")\n",
            "bazel_dep at line 1 has no literal name",
        ),
        (
            "bazel_dep(name = \"rules_rust\", version = \"0.74.0\"\n",
            "MODULE.bazel does not parse at line 2: Parse error: unexpected new line, expected symbol ')'",
        ),
    ] {
        assert_eq!(
            bump.plan_edit(source),
            Err(BumpError::UnsupportedManifest {
                manifest: "MODULE.bazel".to_owned(),
                reason: reason.to_owned(),
            }),
            "{source}"
        );
    }
    assert!(matches!(
        bump.plan_edit("# bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n"),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn npm_layouts_change_only_the_selected_requirement() {
    let bump = BumpRequest::parse("npm:lodash", "4.17.21").expect("npm");
    for (before, after) in [
        (
            r#"{"name":"demo","dependencies":{"lodash":"4.17.20"}}"#,
            r#"{"name":"demo","dependencies":{"lodash":"4.17.21"}}"#,
        ),
        (
            "{\n  \"name\": \"demo\",\n  \"dependencies\": {\n    \"lodash\": \"4.17.20\"\n  }\n}\n",
            "{\n  \"name\": \"demo\",\n  \"dependencies\": {\n    \"lodash\": \"4.17.21\"\n  }\n}\n",
        ),
        (
            "{\"name\":\"lodash\",\"scripts\":{\"lodash\":\"x\"},\"devDependencies\":{\n\"lodash\"\n:\n\"^4\"}}",
            "{\"name\":\"lodash\",\"scripts\":{\"lodash\":\"x\"},\"devDependencies\":{\n\"lodash\"\n:\n\"4.17.21\"}}",
        ),
    ] {
        assert_eq!(bump.plan_edit(before).expect("edit"), after, "{before}");
    }
}

#[test]
fn npm_rejects_ambiguous_malformed_and_unselected_shapes() {
    let bump = BumpRequest::parse("npm:lodash", "4.17.21").expect("npm");
    for source in [
        r#"{"dependencies":{"lodash":"1","lodash":"2"}}"#,
        r#"{"dependencies":{"lodash":"1"},"devDependencies":{"lodash":"2"}}"#,
    ] {
        assert!(
            matches!(bump.plan_edit(source), Err(BumpError::Ambiguous { .. })),
            "{source}"
        );
    }
    for source in [
        r#"{"dependencies":{"lodash":"1"},"dependencies":{"x":"1"}}"#,
        r#"{"dependencies":{"lodash":1}}"#,
        r#"{"dependencies":{"lodash":"1"}"#,
        "[]",
        r#"{"dependencies":[]}"#,
    ] {
        assert!(
            matches!(
                bump.plan_edit(source),
                Err(BumpError::UnsupportedManifest { .. })
            ),
            "{source}"
        );
    }
    for source in [
        r#"{"name":"lodash","scripts":{"lodash":"1"}}"#,
        r#"{"dependencies":{"other":"1"}}"#,
    ] {
        assert!(
            matches!(bump.plan_edit(source), Err(BumpError::NotFound { .. })),
            "{source}"
        );
    }
}

#[test]
fn pin_parser_reads_structural_uses_only() {
    let source = "jobs:\n  call:\n    uses: o/r/.github/workflows/x.yml@v1\n  b:\n    steps:\n      - uses: a/b@v1\n      - run: |\n          uses: z/y@v1\n";
    let parsed = crate::gha::parse_workflow_pins(source).expect("parse");
    let pins: Vec<_> = parsed
        .pins
        .iter()
        .map(|pin| (pin.kind, pin.value.as_str()))
        .collect();
    assert_eq!(
        pins,
        vec![
            (crate::gha::UsesKind::Job, "o/r/.github/workflows/x.yml@v1"),
            (crate::gha::UsesKind::Step, "a/b@v1"),
        ]
    );
    assert!(!parsed.indirect);
    assert!(crate::gha::parse_workflow_pins("a: [").is_err());
    assert_eq!(crate::gha::split_pin("a/b@v1"), Some(("a/b", "v1")));
    assert_eq!(crate::gha::split_pin("a/b@"), Some(("a/b", "")));
    assert_eq!(crate::gha::split_pin("a/b"), None);
    assert_eq!(crate::gha::split_pin("bad action@v1"), None);
}

#[test]
fn commit_response_resolves_sha_or_fails_closed() {
    let sha40 = "3d3c42e5aac5ba805825da76410c181273ba90b1";
    let sha64 = "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08";
    assert_eq!(
        crate::gha::resolve_sha_from_commit_response(&format!(
            "{{\"sha\": \"{sha40}\", \"commit\": {{}}}}"
        ))
        .expect("sha40"),
        sha40
    );
    assert_eq!(
        crate::gha::resolve_sha_from_commit_response(&format!("{{\"sha\": \"{sha64}\"}}"))
            .expect("sha64"),
        sha64
    );
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response(""),
        Err(crate::gha::GhaResolveError::Empty)
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response("not json"),
        Err(crate::gha::GhaResolveError::InvalidJson)
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response("[1, 2]"),
        Err(crate::gha::GhaResolveError::InvalidJson)
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response("{\"commit\": {}}"),
        Err(crate::gha::GhaResolveError::MissingSha)
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response(
            "{\"message\": \"Not Found\", \"documentation_url\": \"https://example.com\"}"
        ),
        Err(crate::gha::GhaResolveError::Upstream { .. })
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response("{\"sha\": \"v5\"}"),
        Err(crate::gha::GhaResolveError::InvalidSha { .. })
    ));
    assert!(matches!(
        crate::gha::resolve_sha_from_commit_response("{\"sha\": null}"),
        Err(crate::gha::GhaResolveError::MissingSha)
    ));
}

#[test]
fn plan_outcome_distinguishes_changed_from_unchanged() {
    let sha = "a".repeat(40);
    let bump = BumpRequest::parse("github-actions:actions/checkout", &sha).expect("request");
    assert!(matches!(
        bump.plan_edit_outcome(&steps("      - uses: actions/checkout@v1\n")).expect("changed"),
        BumpPlanOutcome::Changed(widened) if widened.contains(&sha)
    ));
    assert!(matches!(
        bump.plan_edit_outcome(&steps(&format!("      - uses: actions/checkout@{sha}\n")))
            .expect("unchanged"),
        BumpPlanOutcome::Unchanged
    ));
    let cargo = BumpRequest::parse("cargo:anyhow", "1.2.3").expect("cargo");
    assert!(matches!(
        cargo
            .plan_edit_outcome("[dependencies]\nanyhow = \"1\"\n")
            .expect("changed"),
        BumpPlanOutcome::Changed(_)
    ));
    assert!(matches!(
        cargo
            .plan_edit_outcome("[dependencies]\nanyhow = \"1.2.3\"\n")
            .expect("unchanged"),
        BumpPlanOutcome::Unchanged
    ));
}

#[test]
fn maven_bump_keeps_packaging_and_classifier() {
    let bump = BumpRequest::parse("maven:g:a", "2.0.0").expect("maven");
    for (old, new) in [
        ("g:a:1.0.0", "g:a:2.0.0"),
        ("g:a:jar:1.0.0", "g:a:jar:2.0.0"),
        ("g:a:jar:tests:1.0.0", "g:a:jar:tests:2.0.0"),
    ] {
        for quote in ['"', '\''] {
            let module = maven_module(&format!(
                "        # \"g:a:0.0.1\"\n        \"x:y:1\",\n        {quote}{old}{quote},  # \"g:a:0.0.2\"\n"
            ));
            let expected = maven_module(&format!(
                "        # \"g:a:0.0.1\"\n        \"x:y:1\",\n        {quote}{new}{quote},  # \"g:a:0.0.2\"\n"
            ));
            assert_eq!(bump.plan_edit(&module).expect("edit"), expected);
        }
    }
    let compact =
        format!("{MAVEN_HEAD}maven.install(artifacts = [\"g:a:jar:tests:1.0.0\", \"x:y:1\"])\n");
    assert_eq!(
        bump.plan_edit(&compact).expect("edit"),
        format!("{MAVEN_HEAD}maven.install(artifacts = [\"g:a:jar:tests:2.0.0\", \"x:y:1\"])\n")
    );
}

#[test]
fn maven_bump_rejects_malformed_dynamic_and_ambiguous_without_output() {
    let bump = BumpRequest::parse("maven:g:a", "2.0.0").expect("maven");
    for artifacts in [
        "\"g:a\"",
        "\"g:a:\"",
        "\"g:a:jar:c:1:extra\"",
        "\"g:a::1.0\"",
    ] {
        let module = format!("{MAVEN_HEAD}maven.install(artifacts = [{artifacts}])\n");
        assert!(
            matches!(
                bump.plan_edit(&module),
                Err(BumpError::UnsupportedManifest { .. })
            ),
            "{artifacts}"
        );
    }
    for module in [
        format!("{MAVEN_HEAD}maven.install(artifacts = ARTIFACTS)\n"),
        format!("{MAVEN_HEAD}maven.install(artifacts = [\"g:a:\" + V])\n"),
        format!("{MAVEN_HEAD}maven.install(artifacts = [\"g:a:1.0\"] + EXTRA)\n"),
    ] {
        assert!(
            matches!(
                bump.plan_edit(&module),
                Err(BumpError::UnsupportedManifest { .. })
            ),
            "{module}"
        );
    }
    let multiple = format!(
        "{MAVEN_HEAD}maven.install(artifacts = [\"g:a:1.0\"])\nmaven.install(artifacts = [\"g:a:jar:tests:1.0\"])\n"
    );
    assert!(matches!(
        bump.plan_edit(&multiple),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
    let commented = format!("{MAVEN_HEAD}maven.install(artifacts = [])  # \"g:a:1.0\"\n");
    assert!(matches!(
        bump.plan_edit(&commented),
        Err(BumpError::NotFound { .. })
    ));
}

#[test]
fn go_bump_edits_the_requirement_not_the_comment() {
    let bump = BumpRequest::parse("go:example.com/pkg", "1.2.0").expect("go");
    let input = "module example.com/app\nrequire example.com/pkg v1.0.0 // pinned after v0.9.0\n";
    assert_eq!(
        bump.plan_edit(input).expect("edit"),
        "module example.com/app\nrequire example.com/pkg v1.2.0 // pinned after v0.9.0\n"
    );
}

#[test]
fn go_bump_ignores_replace_and_exclude_and_rejects_malformed_files() {
    let bump = BumpRequest::parse("go:example.com/pkg", "1.2.0").expect("go");
    let input = "module example.com/app\nreplace example.com/pkg => example.com/fork v1.0.0\nexclude example.com/pkg v1.1.0\n";
    assert!(matches!(
        bump.plan_edit(input),
        Err(BumpError::NotFound { .. })
    ));
    assert!(matches!(
        bump.plan_edit("module example.com/app\nrequire (\n\texample.com/pkg v1.0.0\n"),
        Err(BumpError::UnsupportedManifest { .. })
    ));
    let both = "module example.com/app\nrequire example.com/pkg v1.0.0\nrequire (\n\texample.com/pkg v1.1.0\n)\n";
    assert!(matches!(
        bump.plan_edit(both),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
}

#[test]
fn configured_bazel_set_targets_its_own_module_bazel() {
    let bump = BumpRequest::parse_configured(
        "bazel-modules",
        "tools/MODULE.bazel",
        "bazel-modules:rules_cc",
        "0.3.0",
    )
    .expect("configured");
    assert_eq!(bump.set, BumpSet::Bazel);
    assert_eq!(bump.package, "rules_cc");
    assert_eq!(bump.selector, "bazel-modules:rules_cc");
    assert_eq!(bump.target_manifest(), "tools/MODULE.bazel");
    assert!(!bump.needs_update_refresh());
    let builtin = BumpRequest::parse("bazel:rules_cc", "0.3.0").expect("builtin");
    let module = "bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";
    assert_eq!(
        bump.plan_edit(module).expect("widen"),
        "bazel_dep(name = \"rules_cc\", version = \"0.3.0\")\n"
    );
    assert_eq!(bump.plan_edit(module), builtin.plan_edit(module));
}

#[test]
fn configured_bazel_set_rejects_bare_bazelversion_and_bad_names() {
    let parse = |selector: &str, version: &str| {
        BumpRequest::parse_configured("bazel-modules", "tools/MODULE.bazel", selector, version)
    };
    assert_eq!(parse("", "1.0.0"), Err(BumpError::Empty));
    assert_eq!(parse("bazel-modules:rules_cc", ""), Err(BumpError::Empty));
    assert_eq!(
        parse("bazel-modules", "1.0.0"),
        Err(BumpError::BareSet {
            set: "bazel-modules".to_owned()
        })
    );
    for selector in [
        "bazel-modules:",
        "bazel-modules:a:b",
        "bazel-modules:.bazelversion",
        "bazel-modules:bad!",
    ] {
        assert!(
            matches!(
                parse(selector, "1.0.0"),
                Err(BumpError::InvalidPackage { set: "bazel", .. })
            ),
            "{selector}"
        );
    }
    assert!(matches!(
        parse("bazel-modules:rules_cc", "not-a-version"),
        Err(BumpError::Version(_))
    ));
}
