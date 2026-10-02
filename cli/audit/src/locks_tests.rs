use super::*;
use crate::vuln::LockedPackage;

#[test]
fn lock_fallback_parsers_match_normal_paths() {
    for (path, expected) in [
        ("node_modules/outer/deep", Some("deep")),
        ("node_modules/outer/", None),
        ("node_modules/@scope/deep/name", Some("name")),
        ("node_modules/@scope/deep/", None),
    ] {
        assert_eq!(package_lock_name(path).as_deref(), expected, "{path}");
    }
    for key in [
        "@scope/demo@1.0.0",
        "@scope/demo@",
        "@scope/demo",
        "@scope/demo@1.0.0-beta",
    ] {
        assert_eq!(
            split_pnpm_scoped_fallback(key),
            split_pnpm_key(key),
            "{key}"
        );
    }
    for key in ["demo@1.0.0", "demo@", "demo", "@", "demo@1.0.0-beta"] {
        assert_eq!(
            split_pnpm_unscoped_fallback(key),
            split_pnpm_key(key),
            "{key}"
        );
    }
    for line in [
        "Demo (1.0.0)",
        "Demo ()",
        "(1.0.0)",
        "Demo )1.0.0(",
        "Demo",
        "remote: foo (1.0.0)",
        "remote (1.0.0)",
        "remote:foo (1.0.0)",
        "Demo remote (1.0.0)",
        "Demo remote: (1.0.0)",
        "Demo (1.0.0) remote",
        "Demo (1.0.0) (2.0.0)",
    ] {
        assert_eq!(
            split_paket_line_fallback(line),
            split_paket_line(line),
            "{line}"
        );
    }
    assert!(parse_cargo_lock(
        "[[package]]\nname = \"\"\nversion = \"1.0.0\"\nsource = \"registry+test\"\n"
    )
    .is_err());
}

#[test]
fn pnpm_git_resolution_forms_are_never_assessed_as_registry_packages() {
    for resolution in [
        "{type: git}",
        "{commit: deadbeef}",
        "{repo: 'github:owner/repo'}",
        "{tarball: 'https://codeload.github.com/owner/repo/tar.gz/v1'}",
    ] {
        let text = format!("packages:\n  demo@1.0.0:\n    resolution: {resolution}\n");
        let packages = parse_pnpm_lock(&text).expect("pnpm");
        assert_eq!(packages.len(), 1);
        assert!(packages[0].is_git, "{resolution}");
    }
    let packages = parse_pnpm_lock("packages:\n  42: {}\n  'demo@': {}\n  'local@file:../local': {}\n  'linked@link:../local': {}\n  'registry@1.0.0': {}\n").expect("pnpm");
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "registry");
}

#[test]
fn npm_lock_shapes_keep_registry_and_git_entries_but_skip_workspace_links() {
    let packages = parse_package_lock(r#"{
        "packages": {
            "": {"version":"1.0.0"},
            "node_modules/": {},
            "node_modules/@broken": {},
            "node_modules/not-an-object": 1,
            "node_modules/link": {"link":true,"version":"1.0.0"},
            "node_modules/missing-version": {},
            "node_modules/local": {"version":"file:../local"},
            "node_modules/linked": {"version":"link:../local"},
            "node_modules/local-resolved": {"version":"1.0.0","resolved":"file:../local"},
            "node_modules/@scope/demo": {"version":"1.0.0"},
            "node_modules/git-demo": {"version":"1.0.0","from":"github:owner/repo"}
        },
        "dependencies": {
            " ": {"version":"1.0.0"},
            "not-an-object": 1,
            "link": {"link":true,"version":"1.0.0"},
            "missing-version": {},
            "local": {"version":"file:../local"},
            "linked": {"version":"link:../local"},
            "local-resolved": {"version":"1.0.0","resolved":"file:../local"},
            "@scope/demo": {"version":"1.0.0"},
            "git-demo": {"version":"1.0.0","from":"github:owner/repo"},
            "legacy": {"version":"2.0.0","resolved":"https://registry.npmjs.org/legacy/-/legacy-2.0.0.tgz"}
        }
    }"#).expect("npm lock");
    assert_eq!(
        packages
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str(), p.is_git))
            .collect::<Vec<_>>(),
        vec![
            ("@scope/demo", "1.0.0", false),
            ("git-demo", "1.0.0", true),
            ("legacy", "2.0.0", false)
        ]
    );
    assert!(parse_package_lock("[]").is_err());
    assert!(parse_package_lock("{").is_err());
}

#[test]
fn yarn_lock_handles_stanza_boundaries_and_workspace_protocols() {
    let packages = parse_yarn_lock("# yarn lock\n  orphan \"ignored\"\n\nempty@1:\n  resolved \"https://example.com\"\n\nlocal@1:\n  version \"file:../local\"\nlinked@1:\n  version link:../linked\nportal@1:\n  version portal:../portal\nresolved-local@1:\n  version 1.0.0\n  resolved file:../local\nresolved-link@1:\n  version 1.0.0\n  resolved link:../local\nresolved-portal@1:\n  version 1.0.0\n  resolved portal:../local\n\"@scope/demo@^1.0.0\", \"@scope/demo@~1.0.0\":\n  version: \"1.2.0\"\n  resolved \"https://registry.npmjs.org/demo.tgz\"\n# flush\nmissing@1:\n  version \"\"\n  resolved\nregistry:\n  version 2.0.0 # pinned\n  resolved https://registry.npmjs.org/registry.tgz\n").expect("yarn");
    assert_eq!(
        packages
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect::<Vec<_>>(),
        vec![("@scope/demo", "1.2.0"), ("registry", "2.0.0")]
    );
    assert!(packages.iter().all(|p| !p.is_git));
}

#[test]
fn pnpm_documents_reject_scalars_and_nonmapping_packages() {
    for invalid in [
        "42\n",
        "[one, two]\n",
        "packages: [one, two]\n",
        "packages: [\n",
    ] {
        assert!(parse_pnpm_lock(invalid).is_err(), "{invalid}");
    }
    for empty in ["", " \n", "null\n", "---\nnull\n...\n", "packages: null\n"] {
        assert!(
            parse_pnpm_lock(empty).expect("empty lock").is_empty(),
            "{empty}"
        );
    }
}

#[test]
fn cargo_lock_splits_registry_git_and_first_party() {
    let text = r#"
[[package]]
name = "serde"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0000000000000000000000000000000000000000000000000000000000000000"

[[package]]
name = "git-dep"
version = "0.1.0"
source = "git+https://github.com/example/git-dep#abc123"

[[package]]
name = "dx_audit"
version = "0.0.0"
"#;
    let packages = parse_cargo_lock(text).expect("parses");
    assert_eq!(packages.len(), 2);
    let serde = packages
        .iter()
        .find(|package| package.name == "serde")
        .expect("serde");
    assert!(!serde.is_git && !serde.is_private);
    let git = packages
        .iter()
        .find(|package| package.name == "git-dep")
        .expect("git");
    assert!(git.is_git);
    assert!(!packages.iter().any(|package| package.name == "dx_audit"));
}

#[test]
fn cargo_lock_sparse_registry_is_assessable_path_is_skipped() {
    let text = r#"
version = 4

[[package]]
name = "sparse-dep"
version = "1.2.3"
source = "sparse+https://index.crates.io/"
checksum = "1111111111111111111111111111111111111111111111111111111111111111"

[[package]]
name = "path-dep"
version = "0.1.0"
source = "path+file:///tmp/path-dep"

[[package]]
name = "workspace-member"
version = "0.0.0"
"#;
    let packages = parse_cargo_lock(text).expect("parses");
    assert_eq!(packages.len(), 1);
    let sparse = &packages[0];
    assert_eq!(sparse.name, "sparse-dep");
    assert!(!sparse.is_git && !sparse.is_private);
    assert!(!packages.iter().any(|package| package.name == "path-dep"));
    assert!(!packages
        .iter()
        .any(|package| package.name == "workspace-member"));
}

#[test]
fn cargo_lock_malformed_fails_closed() {
    assert!(parse_cargo_lock("not toml [[[ ").is_err());
    assert!(parse_cargo_lock("version = 4\n").is_err());
    let bad_checksum = r#"
[[package]]
name = "serde"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
"#;
    assert!(parse_cargo_lock(bad_checksum).is_err());
}

#[test]
fn pnpm_lock_parses_external_and_skips_links() {
    let text = "lockfileVersion: '9.0'\n\npackages:\n\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n\n  '@astrojs/compiler@4.0.0':\n    resolution: {integrity: sha512-def}\n\n  'my-workspace@link:.':\n    resolution: {directory: .}\n";
    let packages = parse_pnpm_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && package.version == "18.2.0"));
    assert!(packages
        .iter()
        .any(|package| package.name == "@astrojs/compiler" && package.version == "4.0.0"));
    assert!(!packages.iter().any(|package| package.name.contains("link")));
}

#[test]
fn pnpm_lock_handles_quoted_bare_scoped_peer_and_link_keys() {
    let text = "lockfileVersion: '9.0'\npackages:\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n  \"lodash@4.17.21\":\n    resolution: {integrity: sha512-def}\n  acorn@8.18.0:\n    resolution: {integrity: sha512-ghi}\n  '@babel/core@7.29.7':\n    resolution: {integrity: sha512-jkl}\n  'jest@30.2.0(@types/node@22.20.2)':\n    resolution: {integrity: sha512-mno}\n  'my-workspace@link:.':\n    resolution: {directory: .}\n  some-pkg@link:../some-pkg:\n    resolution: {directory: ../some-pkg}\n";
    let packages = parse_pnpm_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && package.version == "18.2.0"));
    assert!(packages
        .iter()
        .any(|package| package.name == "lodash" && package.version == "4.17.21"));
    assert!(packages
        .iter()
        .any(|package| package.name == "acorn" && package.version == "8.18.0"));
    assert!(packages
        .iter()
        .any(|package| package.name == "@babel/core" && package.version == "7.29.7"));
    assert!(packages
        .iter()
        .any(|package| package.name == "jest" && package.version == "30.2.0"));
    assert!(!packages.iter().any(|package| package.name.contains("link")));
    assert!(!packages
        .iter()
        .any(|package| package.version.contains("link:")));
    assert_eq!(packages.len(), 5);
}

#[test]
fn pnpm_lock_missing_packages_is_empty_and_invalid_fails() {
    let missing = "lockfileVersion: '9.0'\nimporters:\n  .:\n    specifier: 1.0.0\n";
    let packages = parse_pnpm_lock(missing).expect("missing packages is empty");
    assert!(packages.is_empty());
    let empty_packages = "packages: {}\n";
    let packages = parse_pnpm_lock(empty_packages).expect("empty packages is empty");
    assert!(packages.is_empty());
    assert!(parse_pnpm_lock("packages: [unclosed").is_err());
    assert!(parse_pnpm_lock("packages: ['a', 'b']").is_err());
}

#[test]
fn pnpm_lock_git_resolutions_are_incomplete_never_dropped() {
    let text = "lockfileVersion: '9.0'\npackages:\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n  'git-dep@github:user/repo#abc123':\n    resolution: {repo: 'https://github.com/user/repo.git', commit: abc123}\n  'typed-git@0.0.0':\n    resolution: {type: git, repo: 'https://github.com/user/typed.git', commit: def456}\n  'tarball-git@https://codeload.github.com/user/repo/tar.gz#abc':\n    resolution: {tarball: 'https://codeload.github.com/user/repo/tar.gz#abc'}\n  'local@file:../local':\n    resolution: {directory: ../local}\n";
    let packages = parse_pnpm_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && !package.is_git));
    for name in ["git-dep", "typed-git", "tarball-git"] {
        let git = packages
            .iter()
            .find(|package| package.name == name)
            .unwrap_or_else(|| panic!("git entry {name}"));
        assert!(git.is_git, "{name} must be git incomplete");
    }
    assert!(!packages.iter().any(|package| package.name == "local"));
    let (findings, unassessed) = crate::vuln::match_packages(&packages, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 3);
    assert!(unassessed
        .iter()
        .all(|entry| entry.reason == crate::vuln::REASON_GIT));
}

#[test]
fn pnpm_lock_merges_multi_document_env_plus_project() {
    let text = "---\nlockfileVersion: '9.0'\npackages:\n  'pnpm-bin@1.0.0':\n    resolution: {integrity: sha512-env}\n---\nlockfileVersion: '9.0'\npackages:\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n";
    let packages = parse_pnpm_lock(text).expect("parses");
    assert!(packages.iter().any(|package| package.name == "react"));
    assert!(packages.iter().any(|package| package.name == "pnpm-bin"));
}

#[test]
fn npm_git_reference_markers_are_explicit_only() {
    assert!(is_npm_git_reference("git+https://github.com/user/repo.git"));
    assert!(is_npm_git_reference("github:user/repo#abc123"));
    assert!(is_npm_git_reference("git@github.com:user/repo.git"));
    assert!(is_npm_git_reference(
        "https://codeload.github.com/user/repo/tar.gz#abc"
    ));
    assert!(is_npm_git_reference(
        "https://github.com/user/repo.git#abc123"
    ));
    assert!(!is_npm_git_reference("18.2.0"));
    assert!(!is_npm_git_reference(
        "https://registry.npmjs.org/react/-/react-18.2.0.tgz"
    ));
    assert!(!is_npm_git_reference(""));
}

#[test]
fn package_lock_parses_registry_skips_links_and_flags_git() {
    let text = r#"{"name":"root","lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/react":{"version":"18.2.0","resolved":"https://registry.npmjs.org/react/-/react-18.2.0.tgz"},"node_modules/@scope/pkg":{"version":"1.0.0","resolved":"https://registry.npmjs.org/@scope/pkg/-/pkg-1.0.0.tgz"},"node_modules/linked":{"version":"1.0.0","link":true},"node_modules/local":{"version":"file:../local"},"node_modules/git-dep":{"version":"github:user/repo#abc123"},"node_modules/git-url":{"version":"1.0.0","resolved":"git+https://github.com/user/repo.git#abc123"}}}"#;
    let packages = parse_package_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && !package.is_git));
    assert!(packages
        .iter()
        .any(|package| package.name == "@scope/pkg" && !package.is_git));
    for name in ["git-dep", "git-url"] {
        let git = packages
            .iter()
            .find(|package| package.name == name)
            .unwrap_or_else(|| panic!("git entry {name}"));
        assert!(git.is_git, "{name} must be git incomplete");
    }
    assert!(!packages.iter().any(|package| package.name == "linked"));
    assert!(!packages.iter().any(|package| package.name == "local"));
    assert!(!packages.iter().any(|package| package.name.is_empty()));
    let (findings, unassessed) = crate::vuln::match_packages(&packages, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 2);
    assert!(unassessed
        .iter()
        .all(|entry| entry.reason == crate::vuln::REASON_GIT));
}

#[test]
fn package_lock_legacy_dependencies_shape_flags_git() {
    let text = r#"{"name":"root","lockfileVersion":1,"dependencies":{"react":{"version":"18.2.0","resolved":"https://registry.npmjs.org/react/-/react-18.2.0.tgz"},"git-dep":{"version":"0.0.0","resolved":"git+ssh://git@github.com/user/repo.git#abc123"}}}"#;
    let packages = parse_package_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && !package.is_git));
    let git = packages
        .iter()
        .find(|package| package.name == "git-dep")
        .expect("git entry");
    assert!(git.is_git);
    assert!(parse_package_lock("not json").is_err());
    assert!(parse_package_lock("[1, 2]").is_err());
}

#[test]
fn yarn_lock_parses_registry_skips_file_and_flags_git() {
    let text = "# yarn lockfile v1\n\nreact@^18.0.0:\n  version \"18.2.0\"\n  resolved \"https://registry.yarnpkg.com/react/-/react-18.2.0.tgz#abc\"\n\n\"@scope/pkg@^1.0.0\":\n  version \"1.0.0\"\n  resolved \"https://registry.yarnpkg.com/@scope/pkg/-/pkg-1.0.0.tgz#def\"\n\n\"git-dep@github:user/repo#abc123\":\n  version \"0.0.0\"\n  resolved \"https://github.com/user/repo.git#abc123\"\n\nlocal@file:../local:\n  version \"file:../local\"\n";
    let packages = parse_yarn_lock(text).expect("parses");
    assert!(packages
        .iter()
        .any(|package| package.name == "react" && package.version == "18.2.0" && !package.is_git));
    assert!(packages
        .iter()
        .any(|package| package.name == "@scope/pkg" && !package.is_git));
    let git = packages
        .iter()
        .find(|package| package.name == "git-dep")
        .expect("git entry");
    assert!(git.is_git);
    assert!(!packages.iter().any(|package| package.name == "local"));
    let (findings, unassessed) = crate::vuln::match_packages(&packages, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 1);
    assert_eq!(unassessed[0].reason, crate::vuln::REASON_GIT);
}

#[test]
fn yarn_lock_dependency_fields_do_not_override_the_entry_version() {
    let text = "x@1:\n  version \"1.0.0\"\n  dependencies:\n    version \"^2.0.0\"\n    resolved \"^3.0.0\"\n";
    let packages = parse_yarn_lock(text).expect("parses");
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version, "1.0.0");
}

#[test]
fn npm_private_packages_are_incomplete_never_clean() {
    let pkgs = vec![
        LockedPackage {
            name: "@internal/pkg".to_owned(),
            version: "1.0.0".to_owned(),
            set: "npm".to_owned(),
            is_git: false,
            is_private: true,
        },
        LockedPackage {
            name: "react".to_owned(),
            version: "18.2.0".to_owned(),
            set: "npm".to_owned(),
            is_git: false,
            is_private: false,
        },
    ];
    let (findings, unassessed) = crate::vuln::match_packages(&pkgs, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 1);
    assert_eq!(unassessed[0].package, "@internal/pkg");
    assert_eq!(unassessed[0].reason, crate::vuln::REASON_PRIVATE);
}

#[test]
fn maven_install_parses_artifacts() {
    let text = r#"{"artifacts": {"junit:junit": {"version": "4.13.2"}, "com.google.guava:guava": {"version": "32.0.0"}}}"#;
    let packages = parse_maven_install(text).expect("parses");
    assert_eq!(packages.len(), 2);
    assert!(packages
        .iter()
        .any(|package| package.name == "junit:junit" && package.version == "4.13.2"));
}

#[test]
fn paket_lock_parses_nuget_section_only() {
    let text = "NUGET\n  remote: https://api.nuget.org/v3/index.json\n    FSharp.Core (10.1.201)\n    My.Pkg (1.2.3)\nHTTP\n  remote: https://example.com\n    Other (9.9.9)\n";
    let packages = parse_paket_lock(text).expect("parses");
    assert_eq!(packages.len(), 2);
    assert!(packages.iter().any(|package| package.name == "FSharp.Core"));
    assert!(!packages.iter().any(|package| package.name == "Other"));
    assert!(packages.iter().all(|package| !package.is_git));
}

#[test]
fn paket_lock_git_section_is_incomplete_never_dropped() {
    let text = "NUGET\n  remote: https://api.nuget.org/v3/index.json\n    FSharp.Core (10.1.201)\nGIT\n  remote: https://github.com/example/lib.git\n    Git.Lib (1.0.0)\nHTTP\n  remote: https://example.com\n    Other (9.9.9)\n";
    let packages = parse_paket_lock(text).expect("parses");
    assert_eq!(packages.len(), 2);
    let assessable = packages
        .iter()
        .find(|package| package.name == "FSharp.Core")
        .expect("nuget entry");
    assert!(!assessable.is_git);
    let git = packages
        .iter()
        .find(|package| package.name == "Git.Lib")
        .expect("git entry");
    assert!(git.is_git);
    assert!(!packages.iter().any(|package| package.name == "Other"));
    let (findings, unassessed) = crate::vuln::match_packages(&packages, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 1);
    assert_eq!(unassessed[0].package, "Git.Lib");
    assert_eq!(unassessed[0].reason, crate::vuln::REASON_GIT);
}

#[test]
fn gemfile_lock_parses_registry_specs_and_skips_dependency_lines() {
    let text = "GEM\n  remote: https://rubygems.org/\n  specs:\n    diff-lcs (1.5.0)\n    rspec (3.13.0)\n      rspec-core (~> 3.13.0)\n      rspec-expectations (~> 3.13.0)\n    rspec-core (3.13.0)\n      rspec-support (~> 3.13.0)\n    rspec-support (3.13.1)\n\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n  rspec (= 3.13.0)\n\nBUNDLED WITH\n   2.5.0\n";
    let packages = parse_gemfile_lock(text).expect("parses");
    assert_eq!(packages.len(), 4);
    let names: Vec<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(names, ["diff-lcs", "rspec", "rspec-core", "rspec-support"]);
    assert!(packages
        .iter()
        .any(|package| package.name == "rspec" && package.version == "3.13.0"));
    assert!(packages.iter().all(|package| package.set == "ruby"));
    assert!(packages
        .iter()
        .all(|package| !package.is_git && !package.is_private));
    assert!(!packages
        .iter()
        .any(|package| package.name == "rspec-support" && package.version.is_empty()));
    assert_eq!(packages[0].version, "1.5.0");
}

#[test]
fn gemfile_lock_keeps_platform_and_prerelease_versions_intact() {
    let text = "GEM\n  remote: https://rubygems.org/\n  specs:\n    nokogiri (1.16.0-x86_64-linux)\n    rspec-support (3.13.1.pre.1)\n";
    let packages = parse_gemfile_lock(text).expect("parses");
    assert_eq!(packages.len(), 2);
    let nokogiri = packages
        .iter()
        .find(|package| package.name == "nokogiri")
        .expect("nokogiri");
    assert_eq!(nokogiri.version, "1.16.0-x86_64-linux");
    let support = packages
        .iter()
        .find(|package| package.name == "rspec-support")
        .expect("rspec-support");
    assert_eq!(support.version, "3.13.1.pre.1");
}

#[test]
fn gemfile_lock_git_and_path_specs_are_incomplete_never_dropped() {
    let text = "GIT\n  remote: https://github.com/example/lib.git\n  revision: 0123456789abcdef\n  specs:\n    git.lib (1.0.0)\n\nPATH\n  remote: ../local\n  specs:\n    local.gem (0.1.0)\n\nGEM\n  remote: https://rubygems.org/\n  specs:\n    diff-lcs (1.5.0)\n";
    let packages = parse_gemfile_lock(text).expect("parses");
    assert_eq!(packages.len(), 3);
    let git = packages
        .iter()
        .find(|package| package.name == "git.lib")
        .expect("git entry");
    assert!(git.is_git);
    let local = packages
        .iter()
        .find(|package| package.name == "local.gem")
        .expect("path entry");
    assert!(local.is_private);
    let (findings, unassessed) = crate::vuln::match_packages(&packages, &[]);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 2);
    assert_eq!(unassessed[0].package, "git.lib");
    assert_eq!(unassessed[0].reason, crate::vuln::REASON_GIT);
    assert_eq!(unassessed[1].package, "local.gem");
    assert_eq!(unassessed[1].reason, crate::vuln::REASON_PRIVATE);
}

#[test]
fn gemfile_lock_missing_gem_section_fails_closed() {
    assert!(parse_gemfile_lock("pnpm-lock.yaml:\n  lockfileVersion: 9\n").is_err());
    assert!(parse_gemfile_lock("").is_err());
    assert!(parse_gemfile_lock("PLATFORMS\n  ruby\n").is_err());
    assert!(parse_gemfile_lock("GEM\n  remote: https://rubygems.org/\n").is_ok());
}

#[test]
fn psgallery_lock_parses_every_pinned_module_as_a_nuget_package() {
    let text = "{\n  \"pwsh\": \"7.5.4\",\n  \"modules\": {\n    \"Pester\": {\n      \"version\": \"5.7.1\",\n      \"gallery\": \"https://www.powershellgallery.com/packages/Pester/5.7.1\"\n    },\n    \"PSScriptAnalyzer\": {\n      \"version\": \"1.25.0\",\n      \"notes\": \"exact module route\"\n    }\n  }\n}\n";
    let packages = parse_psgallery_lock(text).expect("parses");
    assert_eq!(
        packages
            .iter()
            .map(|package| (package.name.as_str(), package.version.as_str()))
            .collect::<Vec<_>>(),
        vec![("PSScriptAnalyzer", "1.25.0"), ("Pester", "5.7.1")]
    );
    assert!(packages
        .iter()
        .all(|package| package.set == "nuget" && !package.is_git && !package.is_private));
}

#[test]
fn psgallery_lock_missing_modules_or_versions_fails_closed() {
    assert!(parse_psgallery_lock("{\"pwsh\": \"7.5.4\"}").is_err());
    assert!(parse_psgallery_lock("{\"modules\": []}").is_err());
    assert!(parse_psgallery_lock("lockfileVersion: 9").is_err());
    assert!(parse_psgallery_lock("").is_err());
    assert!(
        parse_psgallery_lock("{\"modules\": {\"Pester\": {\"notes\": \"floating\"}}}").is_err()
    );
    assert!(parse_psgallery_lock("{\"modules\": {\"Pester\": {\"version\": \"\"}}}").is_err());
    assert!(parse_psgallery_lock("{\"modules\": {}}")
        .expect("an empty lock is valid")
        .is_empty());
}

#[test]
fn go_mod_parses_require_block_and_single_line_with_comments() {
    let text = "module rules_dx/third_party/go\n\ngo 1.24.12\n\nrequire (\n\tgithub.com/bazelbuild/buildtools v0.0.0-20250930140053-2eb4fccefb52 // indirect\n\tgithub.com/google/go-cmp v0.6.0\n\tgithub.com/pmezard/go-difflib v1.0.0\n)\n\nrequire example.com/single v1.2.3 // indirect\n";
    let packages = go_locked_packages(text).expect("parses");
    assert_eq!(packages.len(), 4);
    assert!(packages
        .iter()
        .any(|package| package.name == "github.com/google/go-cmp"
            && package.version == "v0.6.0"
            && package.set == "go"
            && !package.is_git
            && !package.is_private));
    assert!(packages
        .iter()
        .any(|package| package.name == "github.com/bazelbuild/buildtools"
            && package.version == "v0.0.0-20250930140053-2eb4fccefb52"));
    assert!(packages
        .iter()
        .any(|package| package.name == "example.com/single" && package.version == "v1.2.3"));
    assert!(!packages
        .iter()
        .any(|package| package.name == "rules_dx/third_party/go"));
}

#[test]
fn go_mod_replace_paths_skip_while_versioned_replacements_assess() {
    let text = "module example.com/root\n\ngo 1.24.12\n\nrequire (\n\texample.com/local v1.0.0\n\texample.com/forked v1.0.0\n\texample.com/kept v1.0.0\n)\n\nreplace example.com/local => ../local\n\nreplace example.com/forked => example.com/upstream v1.1.0\n";
    let packages = go_locked_packages(text).expect("parses");
    assert!(!packages
        .iter()
        .any(|package| package.name == "example.com/local"));
    let forked = packages
        .iter()
        .find(|package| package.name == "example.com/upstream")
        .expect("versioned replacement assesses");
    assert_eq!(forked.version, "v1.1.0");
    assert!(packages
        .iter()
        .any(|package| package.name == "example.com/kept"));
}

#[test]
fn go_mod_replace_block_paths_skip() {
    let text = "module example.com/root\n\ngo 1.24.12\n\nrequire example.com/local v1.0.0\n\nreplace (\n\texample.com/local => ./local\n)\n";
    let packages = go_locked_packages(text).expect("parses");
    assert!(packages.is_empty());
}

#[test]
fn go_mod_ignores_non_dependency_directives_and_missing_module_fails() {
    let text = "module example.com/root\n\ngo 1.24.12\n\ntoolchain go1.24.12\n\nexclude example.com/bad v1.0.0\n\nretract v1.0.0-bad\n";
    let packages = go_locked_packages(text).expect("parses");
    assert!(packages.is_empty());
    let blocked = "module example.com/root\n\ngo 1.24.12\n\nexclude (\n\texample.com/bad v9.9.9\n)\n\nrequire example.com/kept v1.0.0\n";
    let packages = go_locked_packages(blocked).expect("parses");
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "example.com/kept");
    assert!(go_locked_packages("go 1.24.12\n").is_err());
    assert!(go_locked_packages("").is_err());
    assert!(go_locked_packages("module example.com/root\n)").is_err());
}

#[test]
fn go_mod_keeps_comments_and_urls_out_of_the_parse() {
    let text = "module example.com/root\n\ngo 1.24.12\n\nrequire (\n\thttps://example.com/a v1.0.0 // indirect\n\texample.com/b v1.0.0\n)\n\nreplace https://example.com/a => example.com/a v1.1.0\n";
    let packages = go_locked_packages(text).expect("parses");
    assert_eq!(
        packages
            .iter()
            .map(|package| (package.name.as_str(), package.version.as_str()))
            .collect::<Vec<_>>(),
        vec![("example.com/a", "v1.1.0"), ("example.com/b", "v1.0.0")]
    );
}

#[test]
fn go_mod_incomplete_require_and_replace_lines_fail_closed() {
    let text = "module example.com/root\n\
        require (\n        lone-entry\n    )\n\
        require onlyname\n\
        replace (\n        example.com/a =>\n        example.com/b v1.0.0\n    )\n\
        replace example.com/c =>\n\
        replace example.com/d\n";
    assert!(go_locked_packages(text).is_err());
    let keep = "module example.com/root\n\
        require (\n        lone-entry\n    )\n\
        require example.com/kept v1.0.0\n";
    assert!(go_locked_packages(keep).is_err());
}

#[test]
fn cargo_licenses_prefer_bazel_license_or_unknown() {
    let cargo_lock = r#"
[[package]]
name = "serde"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
    let packages = parse_cargo_lock(cargo_lock).expect("locks");
    let bazel = r#"{"packages": {"serde 1.0.100": {"license": "MIT OR Apache-2.0"}}}"#;
    let licensed = cargo_licenses(bazel, &packages);
    assert_eq!(licensed.len(), 1);
    assert_eq!(licensed[0].license, "MIT OR Apache-2.0");
    let missing = cargo_licenses("not json", &packages);
    assert_eq!(missing[0].license, "UNKNOWN");
}

#[test]
fn unknown_licenses_mark_all_unknown() {
    let packages = vec![LockedPackage {
        name: "react".to_owned(),
        version: "18.2.0".to_owned(),
        set: "npm".to_owned(),
        is_git: false,
        is_private: false,
    }];
    let licensed = unknown_licenses(&packages, "npm");
    assert_eq!(licensed[0].license, "UNKNOWN");
}

#[test]
fn npm_licenses_read_package_lock_fields_or_unknown() {
    let text = r#"{"name":"root","lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/react":{"version":"18.2.0","license":"MIT"},"node_modules/@scope/pkg":{"version":"1.0.0","license":"Apache-2.0"},"node_modules/nolicense":{"version":"2.0.0"}}}"#;
    let packages = parse_package_lock(text).expect("locks");
    let licensed = npm_licenses(text, &packages);
    let react = licensed
        .iter()
        .find(|entry| entry.name == "react")
        .expect("react");
    assert_eq!(react.license, "MIT");
    let scoped = licensed
        .iter()
        .find(|entry| entry.name == "@scope/pkg")
        .expect("scoped");
    assert_eq!(scoped.license, "Apache-2.0");
    let missing = licensed
        .iter()
        .find(|entry| entry.name == "nolicense")
        .expect("unlicensed");
    assert_eq!(missing.license, "UNKNOWN");
}

#[test]
fn npm_licenses_read_legacy_dependencies_shape() {
    let text = r#"{"name":"root","lockfileVersion":1,"dependencies":{"react":{"version":"18.2.0","license":"MIT"},"nolicense":{"version":"2.0.0"}}}"#;
    let packages = parse_package_lock(text).expect("locks");
    let licensed = npm_licenses(text, &packages);
    assert_eq!(
        licensed
            .iter()
            .find(|entry| entry.name == "react")
            .expect("react")
            .license,
        "MIT"
    );
    assert_eq!(
        licensed
            .iter()
            .find(|entry| entry.name == "nolicense")
            .expect("nolicense")
            .license,
        "UNKNOWN"
    );
    let fallback = npm_licenses("not json", &packages);
    assert!(fallback.iter().all(|entry| entry.license == "UNKNOWN"));
}

#[test]
fn license_extractors_skip_malformed_entries_without_inventing_ids() {
    let cargo_lock = "[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
    let cargo_packages = parse_cargo_lock(cargo_lock).expect("locks");
    let blank = r#"{"packages": {"serde 1.0.100": {"license": "  "}}}"#;
    assert_eq!(cargo_licenses(blank, &cargo_packages)[0].license, "UNKNOWN");

    let text = r#"{"packages":{"node_modules/str":"1.0.0","node_modules/nov":{"license":"MIT"},"node_modules/nolic":{"version":"1.0.0","license":" "}},"dependencies":{"  ":{"version":"1.0.0"},"obj":"1.0.0","nover":{"license":"MIT"},"nolic":{"version":"1.0.0","license":""}}}"#;
    let licensed = npm_licenses(text, &[]);
    assert!(licensed.is_empty());
    assert!(npm_licenses("[]", &[]).is_empty());
}

#[test]
fn inventory_licenses_resolve_per_ecosystem_with_version_scope() {
    use crate::license_policy::LicenseInventory;
    let inventory = vec![
        LicenseInventory {
            package: "react".to_owned(),
            set: "npm".to_owned(),
            license: "MIT".to_owned(),
            versions: "18.2.0".to_owned(),
            text_present: true,
        },
        LicenseInventory {
            package: "junit:junit".to_owned(),
            set: "maven".to_owned(),
            license: "EPL-1.0".to_owned(),
            versions: "[4.0,5.0)".to_owned(),
            text_present: false,
        },
        LicenseInventory {
            package: "FSharp.Core".to_owned(),
            set: "nuget".to_owned(),
            license: "MIT".to_owned(),
            versions: "10.1.201".to_owned(),
            text_present: true,
        },
        LicenseInventory {
            package: "github.com/google/go-cmp".to_owned(),
            set: "go".to_owned(),
            license: "BSD-3-Clause".to_owned(),
            versions: "v0.6.0".to_owned(),
            text_present: false,
        },
    ];
    let npm_pkgs = vec![LockedPackage {
        name: "react".to_owned(),
        version: "18.2.0".to_owned(),
        set: "npm".to_owned(),
        is_git: false,
        is_private: false,
    }];
    let licensed = inventory_licenses(&npm_pkgs, &inventory, "npm");
    assert_eq!(licensed[0].license, "MIT");
    assert!(inventory_text_present(&npm_pkgs[0], &inventory));
    let upgraded = LockedPackage {
        version: "19.0.0".to_owned(),
        ..npm_pkgs[0].clone()
    };
    let licensed = inventory_licenses(std::slice::from_ref(&upgraded), &inventory, "npm");
    assert_eq!(licensed[0].license, "UNKNOWN");
    assert!(!inventory_text_present(&upgraded, &inventory));
    let maven = LockedPackage {
        name: "junit:junit".to_owned(),
        version: "4.13.2".to_owned(),
        set: "maven".to_owned(),
        is_git: false,
        is_private: false,
    };
    assert_eq!(
        inventory_licenses(std::slice::from_ref(&maven), &inventory, "maven")[0].license,
        "EPL-1.0"
    );
    let maven_out = LockedPackage {
        version: "5.0.0".to_owned(),
        ..maven.clone()
    };
    assert_eq!(
        inventory_licenses(std::slice::from_ref(&maven_out), &inventory, "maven")[0].license,
        "UNKNOWN"
    );
    let unknown = LockedPackage {
        name: "other".to_owned(),
        version: "1.0.0".to_owned(),
        set: "go".to_owned(),
        is_git: false,
        is_private: false,
    };
    assert_eq!(
        inventory_licenses(std::slice::from_ref(&unknown), &inventory, "go")[0].license,
        "UNKNOWN"
    );
    assert!(!inventory_text_present(&unknown, &inventory));
}

#[test]
fn regex_pnpm_scoped_peer_and_dash_boundaries() {
    assert_eq!(
        split_pnpm_key("@babel/core@7.29.7(@babel/types@7.0.0)"),
        Some(("@babel/core".to_owned(), "7.29.7".to_owned()))
    );
    assert_eq!(
        split_pnpm_key("my-jest@30.2.0"),
        Some(("my-jest".to_owned(), "30.2.0".to_owned()))
    );
    assert_eq!(
        split_pnpm_key("jest@30.2.0"),
        Some(("jest".to_owned(), "30.2.0".to_owned()))
    );
    assert_eq!(
        split_pnpm_key("some-pkg@link:../some-pkg"),
        Some(("some-pkg".to_owned(), "link:../some-pkg".to_owned()))
    );
}

#[test]
fn regex_paket_line_keeps_greedy_paren_and_remote_guard() {
    assert_eq!(
        split_paket_line("My.Pkg (1.2.3)"),
        Some(("My.Pkg".to_owned(), "1.2.3".to_owned()))
    );
    assert_eq!(
        split_paket_line("My.Pkg (1.2.3) extra"),
        Some(("My.Pkg".to_owned(), "1.2.3".to_owned()))
    );
    assert!(split_paket_line("remote: foo (1.2.3)").is_none());
    assert!(split_paket_line("no-parens-here").is_none());
}
