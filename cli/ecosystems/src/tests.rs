use std::collections::BTreeSet;

use super::*;

#[test]
fn shipped_table_has_every_ecosystem() {
    assert_eq!(rows().len(), 23);
    assert_eq!(rows().len(), Ecosystem::ALL.len());
}

#[test]
fn rows_are_complete_and_unique() {
    let mut seen = BTreeSet::new();
    for row in rows() {
        assert!(!row.name.is_empty());
        assert!(!row.manifest.is_empty(), "{} has no manifest", row.name);
        assert!(
            row.alt_manifests
                .iter()
                .all(|alt| !alt.is_empty() && *alt != row.manifest),
            "{} has a bad alternate manifest",
            row.name
        );
        assert!(seen.insert(row.name.as_str()), "duplicate {}", row.name);
        assert!(
            !row.lock_required || !row.locks.is_empty(),
            "{} requires a lock it does not name",
            row.name
        );
        assert!(
            !row.license_audit || row.audit,
            "{} audits licenses but not advisories",
            row.name
        );
        assert!(
            Ecosystem::parse(&row.name).is_some(),
            "{} unparsed",
            row.name
        );
    }
}

#[test]
fn every_variant_has_a_row() {
    for ecosystem in Ecosystem::ALL {
        assert!(ecosystem.row().is_some(), "{} has no row", ecosystem.name());
        assert_eq!(Ecosystem::parse(ecosystem.name()), Some(ecosystem));
    }
}

#[test]
fn supported_list_matches_rows() {
    let names: Vec<&str> = rows().iter().map(|row| row.name.as_str()).collect();
    assert_eq!(SUPPORTED_ECOSYSTEMS, names.join(", "));
}

#[test]
fn unknown_name_is_rejected() {
    let text = TABLE.replacen("\"uv\"", "\"nope\"", 1);
    assert!(parse_rows(&text).is_err());
}

#[test]
fn row_columns_resolve() {
    assert!(Ecosystem::Cargo.license_audit() && !Ecosystem::Uv.license_audit());
    assert!(Ecosystem::Zig.lock_required() && !Ecosystem::Cargo.lock_required());
    assert_eq!(Ecosystem::PowerShell.name(), "powershell");
    assert_eq!(Ecosystem::parse("swiftpm"), Some(Ecosystem::SwiftPm));
    assert_eq!(Ecosystem::parse("Uv"), None);
}

#[test]
fn missing_field_is_rejected() {
    let text = r#"{"ecosystems":[{"name":"uv","manifest":"pyproject.toml"}]}"#;
    assert!(parse_rows(text).is_err());
}

#[test]
fn false_flags_report_pinned() {
    assert_eq!(Ecosystem::Bazel.supported_check(), "pinned");
    assert_eq!(Ecosystem::Uv.supported_check(), "check");
    assert_eq!(Ecosystem::Dub.advisory_family(), None);
}

#[test]
fn lock_and_source_columns_match_the_audit_column() {
    for row in rows() {
        assert!(
            row.locks
                .iter()
                .all(|name| !name.is_empty() && !name.contains('/'))
        );
        if row.advisory_family.is_none() {
            assert_eq!(
                row.osv_source, None,
                "{} has a source but no family",
                row.name
            );
        }
        if let Some(url) = &row.osv_source {
            assert!(url.starts_with("https://"), "{} source {url}", row.name);
        }
        if row.audit && row.advisory_family.is_some() {
            assert!(
                row.osv_source.is_some(),
                "{} audited without source",
                row.name
            );
        }
    }
}

#[test]
fn lookups_resolve_rows() {
    assert_eq!(Ecosystem::Hex.lockfiles(), ["mix.lock", "rebar.lock"]);
    assert_eq!(Ecosystem::Npm.lockfiles()[0], "pnpm-lock.yaml");
    assert_eq!(Ecosystem::Maven.lockfiles()[0], "maven_install.json");
    assert_eq!(advisory_family_named("rubygems"), Some("rubygems"));
    assert_eq!(advisory_family_named("ruby"), None);
    assert_eq!(advisory_family_named("unknown-set"), None);
    assert_eq!(
        osv_source_of_family("cargo"),
        Some("https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip")
    );
    assert_eq!(osv_source_of_family("nope"), None);
}

#[test]
fn every_language_row_is_complete() {
    assert_eq!(language_rows().len(), Language::ALL.len());
    for language in Language::ALL {
        assert!(!language.name().is_empty(), "{language:?} unnamed");
        assert_eq!(Language::parse(language.name()), Some(language));
        assert!(!language.directory().is_empty(), "{language:?} has no row");
        assert!(
            language == Language::Cc || !language.ecosystems().is_empty(),
            "{language:?} lists no ecosystem"
        );
    }
    assert_eq!(Language::parse("bogus"), None);
}

#[test]
fn language_columns_resolve() {
    assert_eq!(Language::Js.name(), "js");
    assert_eq!(Language::Js.directory(), "javascript");
    assert_eq!(Language::Ts.directory(), "typescript");
    assert!(Language::Rust.uses_semver_grammar());
    assert!(!Language::Python.uses_semver_grammar());
    assert_eq!(Language::Kotlin.ecosystems(), [Ecosystem::Maven]);
    assert_eq!(
        Language::of_ecosystem(Ecosystem::Hex, None),
        Some(Language::Elixir)
    );
    assert_eq!(
        Language::of_ecosystem(Ecosystem::Hex, Some(Language::Erlang)),
        Some(Language::Erlang)
    );
    assert_eq!(
        Language::of_ecosystem(Ecosystem::Cargo, Some(Language::Erlang)),
        Some(Language::Rust)
    );
    assert_eq!(Language::of_ecosystem(Ecosystem::Bazel, None), None);
}

#[test]
fn manifest_columns_resolve() {
    assert_eq!(Ecosystem::Dub.alt_manifests(), ["dub.sdl"]);
    assert_eq!(Ecosystem::Hex.alt_manifests(), ["rebar.config"]);
    assert!(Ecosystem::Uv.alt_manifests().is_empty());
    assert!(Ecosystem::Maven.shared_manifest());
    assert!(Ecosystem::Ansible.shared_manifest());
    assert!(Ecosystem::Bicep.shared_manifest());
    assert!(!Ecosystem::Cargo.shared_manifest());
}

#[test]
fn backend_columns_name_a_module_or_pinned() {
    for row in rows() {
        for backend in [&row.plan_backend, &row.check_backend] {
            assert!(
                backend == "pinned"
                    || backend == &row.name
                    || (row.name == "pub" && backend == "pub_dev"),
                "{} names backend {backend}",
                row.name
            );
        }
    }
    assert!(Ecosystem::Maven.graph_only() && Ecosystem::Nuget.graph_only());
    assert!(!Ecosystem::Cargo.graph_only());
}

#[test]
fn json_states_no_name_the_registry_states() {
    let raw = serde_json::from_str::<Table>(TABLE)
        .expect("table")
        .ecosystems;
    for row in raw {
        let Some(hub) = dx_families::hub(&row.name) else {
            continue;
        };
        assert_ne!(
            row.manifest, hub.manifest,
            "{} repeats its manifest",
            row.name
        );
        if hub.lock.is_none() {
            assert_ne!(
                row.lockfiles,
                Some(Vec::new()),
                "{} repeats its absent lock",
                row.name
            );
        }
        if let Some(lock) = &hub.lock {
            assert_ne!(
                row.lockfiles,
                Some(vec![lock.clone()]),
                "{} repeats its lock",
                row.name
            );
        }
    }
}

#[test]
fn registry_names_merge_manifest_lock_and_state() {
    assert_eq!(
        Ecosystem::Composer.registry_names(),
        ["composer.json", "composer.lock", "composer.lock.json"]
    );
    assert_eq!(Ecosystem::LuaRocks.registry_names(), ["luarocks.lock.json"]);
}
