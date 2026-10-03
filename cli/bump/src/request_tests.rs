use super::*;

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
fn fallback_edits_preserve_the_same_bytes_as_regex_edits() {
    for input in [
        "require example.com/demo v1.2.3\n",
        " example.com/v2/demo v2.3.4-beta+build // indirect\r\n",
        "example.com/demo v1.2.3+build",
        "example.com/demo vbroken",
        "example.com/demo v123",
        "example.com/demo v...",
        "example.com/demo v1.2.3 suffix",
    ] {
        assert_eq!(
            replace_go_version_token_fallback(input, "v3.0.0"),
            replace_go_version_token(input, "v3.0.0"),
            "{input}"
        );
    }
    for input in [
        "version = \"1.0.0\"",
        "version=\"1.0.0\" # keep",
        "no attribute",
        "version",
        "version = 1",
        "version = \"unterminated",
    ] {
        assert_eq!(
            replace_version_attr_fallback(input, "2.0.0"),
            replace_version_attr(input, "2.0.0"),
            "{input}"
        );
    }
    for input in [
        "\"demo\": \"1.0.0\",",
        "\"demo\":\"1.0.0\"",
        "missing colon",
        "\"demo\": 1",
        "\"demo\": \"unterminated",
    ] {
        assert_eq!(
            replace_first_quoted_version_after_colon_fallback(input, "2.0.0"),
            replace_first_quoted_version_after_colon(input, "2.0.0"),
            "{input}"
        );
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
            "{\n\"demo\": 1\n}\n",
        ),
        (
            "go:example.com/demo",
            "module example.com/root\n",
            "require example.com/demo v1.0.0\nrequire example.com/demo v1.1.0\n",
            "require example.com/demo vbad\n",
        ),
        (
            "maven:org.example:demo",
            "maven.install(artifacts = [])\n",
            "\"org.example:demo:1.0.0\",\n\"org.example:demo:1.1.0\",\n",
            "\"org.example:demo:\",\n",
        ),
        (
            "nuget:Demo",
            "source https://example.com\n",
            "nuget Demo 1.0.0\nnuget Demo 1.1.0\n",
            "nuget Demo\n",
        ),
    ] {
        let bump = BumpRequest::parse(selector, "2.0.0").expect("request");
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
        ("go:example.com/demo", "// example.com/demo v9.0.0\n# keep\nrequire (\n example.com/demo-extra v1.0.0\n example.com/demo v1.2.3-beta+build // indirect\n)\n", "// example.com/demo v9.0.0\n# keep\nrequire (\n example.com/demo-extra v1.0.0\n example.com/demo v2.0.0 // indirect\n)\n"),
        ("nuget:Demo", "# nuget Demo 9.0.0\r\n// keep\r\nnuget Demo.Other 1.0.0\r\n  nuget Demo 1.2.3  # pinned\r\n", "# nuget Demo 9.0.0\r\n// keep\r\nnuget Demo.Other 1.0.0\r\n  nuget Demo 2.0.0  # pinned\r\n"),
        ("nuget:Demo", "nuget Demo 1.2.3", "nuget Demo 2.0.0"),
    ] {
        let bump = BumpRequest::parse(selector, "2.0.0").expect("request");
        assert_eq!(bump.plan_edit(input).expect("edit"), expected);
    }
}

#[test]
fn workflow_widen_requires_one_nonempty_pin() {
    let sha = "a".repeat(40);
    let bump = BumpRequest::parse("gha:actions/checkout", &sha).expect("request");
    assert_eq!(bump.target_manifest(), ".github/workflows");
    assert!(matches!(
        bump.plan_edit("steps: []\n"),
        Err(BumpError::NotFound { manifest, .. }) if manifest == ".github/workflows"
    ));
    assert!(matches!(
        bump.plan_edit("- uses: actions/checkout@\n"),
        Err(BumpError::UnsupportedManifest { manifest, .. }) if manifest == ".github/workflows"
    ));
    assert!(matches!(
        bump.plan_edit("- uses: actions/checkout@v1\n- uses: actions/checkout@v2\n"),
        Err(BumpError::Ambiguous { count: 2, manifest, .. }) if manifest == ".github/workflows"
    ));
    assert_eq!(
        bump.plan_edit("- uses: actions/checkout@v1").expect("edit"),
        format!("- uses: actions/checkout@{sha}")
    );
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
fn aliases_resolve_to_canonical_sets() {
    let bump = BumpRequest::parse("gomod:example.com/mod", "1.2.3").expect("gomod");
    assert_eq!(bump.set, BumpSet::Go);
    let bump = BumpRequest::parse("gha:actions/checkout", "v4").expect("gha");
    assert_eq!(bump.set, BumpSet::GithubActions);
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

    let module = "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n        \"org.junit.jupiter:junit-jupiter-api:6.1.3\",\n    ],\n)\n";
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.3").expect("maven");
    let widened = bump.plan_edit(module).expect("edit");
    assert!(widened.contains("\"junit:junit:4.13.3\""), "{widened}");
    assert!(
        widened.contains("\"org.junit.jupiter:junit-jupiter-api:6.1.3\""),
        "{widened}"
    );
    assert!(matches!(
        bump.plan_edit("maven.install(\n    artifacts = [\n    ],\n)\n"),
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

#[test]
fn maven_and_nuget_edits_fail_closed_on_ambiguous_and_prefixes() {
    let module = "        \"junit:junit:4.13.2\",\n        \"junit:junit:4.13.2\",\n";
    let bump = BumpRequest::parse("maven:junit:junit", "4.13.3").expect("maven");
    assert!(matches!(
        bump.plan_edit(module),
        Err(BumpError::Ambiguous { count: 2, .. })
    ));
    let module = "        \"junit:junit-jupiter:1.0.0\",\n";
    assert!(matches!(
        bump.plan_edit(module),
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
    let workflow = format!("      - uses: actions/checkout@{old} # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n", old = "3d3c42e5aac5ba805825da76410c181273ba90b1");
    let bump = BumpRequest::parse("github-actions:actions/checkout", &sha).expect("sha");
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
fn regex_go_module_token_respects_boundaries() {
    assert!(line_contains_module_token(
        "require example.com/mod v1.2.3",
        "example.com/mod"
    ));
    assert!(!line_contains_module_token(
        "require example.com/mod-extra v1.2.3",
        "example.com/mod"
    ));
    assert!(!line_contains_module_token(
        "require example.com/modx v1.2.3",
        "example.com/mod"
    ));
}

#[test]
fn regex_go_version_token_replaces_last_and_keeps_suffix() {
    let line = "require example.com/mod v1.2.3 // keep\n";
    let replaced = replace_go_version_token(line, "v1.3.0").expect("replace");
    assert!(replaced.contains("v1.3.0"), "{replaced}");
    assert!(replaced.contains("// keep"), "{replaced}");

    let pre = "require example.com/mod v1.2.3-alpha+001\n";
    let replaced = replace_go_version_token(pre, "v1.3.0").expect("pre");
    assert!(replaced.contains("v1.3.0"), "{replaced}");
    assert!(!replaced.contains("alpha"), "{replaced}");

    assert!(replace_go_version_token("require example.com/mod v1\n", "v2.0.0").is_none());
}

#[test]
fn regex_cargo_boundary_prefers_exact_table_key() {
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
fn regex_version_attr_and_json_keep_spacing() {
    let tight = "bazel_dep(name = \"rules_rust\",version=\"0.74.0\")\n";
    let replaced = replace_version_attr(tight, "0.75.0").expect("tight");
    assert!(replaced.contains("version=\"0.75.0\""), "{replaced}");

    let spaced = "bazel_dep(name = \"rules_rust\", version   =   \"0.74.0\")\n";
    let replaced = replace_version_attr(spaced, "0.75.0").expect("spaced");
    assert!(replaced.contains("version   =   \"0.75.0\""), "{replaced}");

    let line = "    \"jest\": \"30.2.0\",";
    let replaced = replace_first_quoted_version_after_colon(line, "30.3.0").expect("json");
    assert!(replaced.contains("\"jest\": \"30.3.0\""), "{replaced}");
}
