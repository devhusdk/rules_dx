use super::*;

fn pairs(file: &GoModFile) -> Vec<(&str, &str)> {
    file.require
        .iter()
        .map(|requirement| (requirement.module.as_str(), requirement.version.as_str()))
        .collect()
}

#[test]
fn mod_reads_require_blocks_and_single_lines() {
    let text = "module rules_dx/third_party/go\n\ngo 1.24.12\n\nrequire (\n\tgithub.com/bazelbuild/buildtools v0.0.0-20250930140053-2eb4fccefb52 // indirect\n\tgithub.com/google/go-cmp v0.6.0\n\tgithub.com/pmezard/go-difflib v1.0.0\n)\n\nrequire example.com/single v1.2.3 // indirect\n";
    let file = parse_mod(text).expect("parses");
    assert_eq!(file.module, "rules_dx/third_party/go");
    assert_eq!(file.go.as_deref(), Some("1.24.12"));
    assert_eq!(
        pairs(&file),
        vec![
            (
                "github.com/bazelbuild/buildtools",
                "v0.0.0-20250930140053-2eb4fccefb52"
            ),
            ("github.com/google/go-cmp", "v0.6.0"),
            ("github.com/pmezard/go-difflib", "v1.0.0"),
            ("example.com/single", "v1.2.3"),
        ]
    );
    let indirect = file
        .require
        .iter()
        .map(|requirement| requirement.indirect)
        .collect::<Vec<_>>();
    assert_eq!(indirect, vec![true, false, false, true]);
}

#[test]
fn mod_reads_replace_targets_in_both_forms() {
    let text = "module example.com/root\n\ngo 1.24.12\n\nrequire (\n\texample.com/local v1.0.0\n\texample.com/forked v1.0.0\n)\n\nreplace example.com/local => ../local\n\nreplace (\n\texample.com/forked => example.com/upstream v1.1.0\n\texample.com/pinned v1.0.0 => example.com/pinned v1.2.0\n)\n";
    let file = parse_mod(text).expect("parses");
    let targets = file
        .replace
        .iter()
        .map(|replacement| (replacement.module.as_str(), &replacement.target))
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 3);
    assert_eq!(
        targets[0],
        (
            "example.com/local",
            &GoReplacementTarget::FilePath("../local".to_owned())
        )
    );
    assert_eq!(
        targets[1],
        (
            "example.com/forked",
            &GoReplacementTarget::Module {
                module: "example.com/upstream".to_owned(),
                version: "v1.1.0".to_owned(),
            }
        )
    );
    assert_eq!(file.replace[2].version.as_deref(), Some("v1.0.0"));
}

#[test]
fn mod_keeps_a_url_scheme_inside_a_replace_path() {
    let text = "module example.com/root\n\nrequire example.com/a v1.0.0\n\nreplace https://example.com/a => example.com/a v2.0.0\n";
    let file = parse_mod(text).expect("parses");
    assert_eq!(file.replace[0].module, "https://example.com/a");
    assert_eq!(
        file.replace[0].target,
        GoReplacementTarget::Module {
            module: "example.com/a".to_owned(),
            version: "v2.0.0".to_owned(),
        }
    );
}

#[test]
fn mod_reports_directives_that_are_not_requirements_separately() {
    let text = "module example.com/root\n\ngo 1.24.12\n\ntoolchain go1.24.12\n\nignore ./vendor\n\nexclude example.com/bad v1.0.0\n\nexclude (\n\texample.com/worse v9.9.9\n)\n\nretract v1.0.0-bad\n\nretract [v1.1.0, v1.2.0]\n";
    let file = parse_mod(text).expect("parses");
    assert!(file.require.is_empty());
    assert!(file.replace.is_empty());
}

#[test]
fn mod_without_a_module_directive_reads_as_empty() {
    let file = parse_mod("go 1.24.12\n").expect("parses");
    assert_eq!(file.module, "");
    assert_eq!(parse_mod("").expect("parses"), GoModFile::default());
}

#[test]
fn mod_malformed_lines_fail_instead_of_being_dropped() {
    for text in [
        "module example.com/root\nrequire (\nlone-entry\n)\n",
        "module example.com/root\nrequire onlyname\n",
        "module example.com/root\nreplace example.com/c =>\n",
        "module example.com/root\nreplace example.com/d\n",
        "module example.com/root\n)",
        "modulegithub.com/no-space\n",
    ] {
        assert!(parse_mod(text).is_err(), "{text}");
    }
}

#[test]
fn mod_attaches_each_marker_to_its_own_line() {
    let text = "module example.com/root\n\nrequire (\n\texample.com/greet v1.0.0\n\texample.com/dev v0.1.0 // depcheck:test\n\texample.com/opt v0.2.0 // optional\n\texample.com/winonly v0.3.0 // depcheck:platform\n\texample.com/old v0.4.0 // indirect\n)\n\nrequire example.com/single v1.2.3 // depcheck:test\n";
    let file = parse_mod(text).expect("parses");
    let markers = file
        .require
        .iter()
        .map(|requirement| (requirement.module.as_str(), requirement.marker.as_deref()))
        .collect::<Vec<_>>();
    assert_eq!(
        markers,
        vec![
            ("example.com/greet", None),
            ("example.com/dev", Some("depcheck:test")),
            ("example.com/opt", Some("optional")),
            ("example.com/winonly", Some("depcheck:platform")),
            ("example.com/old", Some("indirect")),
            ("example.com/single", Some("depcheck:test")),
        ]
    );
}

#[test]
fn mod_ignores_comments_that_belong_to_other_directives() {
    let text = "module example.com/root\n\n// depcheck:test\n\ngodebug default=go1.21 // depcheck:optional\n\ntool example.com/cmd/tool // depcheck:platform\n\nrequire (\n\texample.com/a v1.0.0\n\texample.com/b v1.0.0 // depcheck:test\n)\n\nexclude (\n\texample.com/a v9.9.9 // depcheck:platform\n)\n";
    let file = parse_mod(text).expect("parses");
    let markers = file
        .require
        .iter()
        .map(|requirement| (requirement.module.as_str(), requirement.marker.as_deref()))
        .collect::<Vec<_>>();
    assert_eq!(
        markers,
        vec![
            ("example.com/a", None),
            ("example.com/b", Some("depcheck:test")),
        ]
    );
}

#[test]
fn sum_keeps_the_first_version_of_each_module() {
    let text = "# header\n\nexample.com/greet v1.0.0 h1:abc\nsolo-token\nexample.com/greet v1.0.0/go.mod h1:def\nexample.com/greet v2.0.0 h1:ghi\n";
    let packages = parse_sum(text);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages["example.com/greet"], "v1.0.0");
    assert!(parse_sum("").is_empty());
}

fn set(text: &str, module: &str) -> Result<String, GoEditError> {
    set_require_version(text, module, "v1.2.0")
}

#[test]
fn set_changes_the_requirement_and_not_the_comment() {
    let text = "module example.com/app\nrequire example.com/pkg v1.0.0 // pinned after v0.9.0\n";
    assert_eq!(
        set(text, "example.com/pkg").expect("edit"),
        "module example.com/app\nrequire example.com/pkg v1.2.0 // pinned after v0.9.0\n"
    );
}

#[test]
fn set_edits_block_entries_and_keeps_markers_order_and_other_directives() {
    let text = "module example.com/app\n\ngo 1.24\n\nrequire (\n\texample.com/a v1.0.0 // indirect\n\texample.com/pkg   v1.0.0   // v0.1.0 v0.2.0\n\texample.com/z v3.0.0\n)\n\nreplace example.com/pkg v1.0.0 => example.com/fork v1.0.0\nexclude example.com/pkg v1.1.0\nretract v1.0.0 // v1.0.0\n";
    let out = set(text, "example.com/pkg").expect("edit");
    assert_eq!(
        out,
        text.replace(
            "example.com/pkg   v1.0.0   // v0.1.0 v0.2.0",
            "example.com/pkg   v1.2.0   // v0.1.0 v0.2.0"
        )
    );
    let file = parse_mod(&out).expect("reparse");
    assert_eq!(file.replace.len(), 1);
    assert!(file.require[0].indirect);
}

#[test]
fn set_handles_quoted_paths_and_versions_and_crlf() {
    let text = "module example.com/app\r\nrequire \"example.com/pkg\" \"v1.0.0\" // x\r\nrequire (\r\n\t`example.com/b` v1.0.0\r\n)\r\n";
    let out = set(text, "example.com/pkg").expect("edit");
    assert!(
        out.contains("\"example.com/pkg\" \"v1.2.0\" // x\r\n"),
        "{out}"
    );
    let out = set(text, "example.com/b").expect("edit");
    assert!(out.contains("`example.com/b` v1.2.0\r\n"), "{out}");
}

#[test]
fn set_ignores_other_directives_that_name_the_module() {
    let text = "module example.com/app\nrequire example.com/other v1.0.0\nreplace example.com/pkg => example.com/fork v1.0.0\nexclude example.com/pkg v1.0.0\n";
    assert_eq!(set(text, "example.com/pkg"), Err(GoEditError::NotFound));
}

#[test]
fn set_rejects_duplicate_requirements() {
    let text = "module example.com/app\nrequire example.com/pkg v1.0.0\nrequire (\n\texample.com/pkg v1.1.0\n)\n";
    assert_eq!(set(text, "example.com/pkg"), Err(GoEditError::Ambiguous(2)));
}

#[test]
fn set_rejects_malformed_files_and_versions() {
    assert!(matches!(
        set(
            "module example.com/app\nrequire example.com/pkg\n",
            "example.com/pkg"
        ),
        Err(GoEditError::Invalid(_))
    ));
    let text = "module example.com/app\nrequire example.com/pkg v1.0.0\n";
    assert!(matches!(
        set_require_version(text, "example.com/pkg", "v1 2"),
        Err(GoEditError::Invalid(_))
    ));
    assert!(matches!(
        set_require_version(text, "example.com/pkg", ""),
        Err(GoEditError::Invalid(_))
    ));
}

fn target_for<'a>(
    file: &'a GoModFile,
    module: &str,
    version: &str,
) -> Option<&'a GoReplacementTarget> {
    file.replacement_for(module, version)
        .map(|replacement| &replacement.target)
}

fn upstream(module: &str, version: &str) -> GoReplacementTarget {
    GoReplacementTarget::Module {
        module: module.to_owned(),
        version: version.to_owned(),
    }
}

#[test]
fn replacement_for_matches_version_qualified_directives_only_on_that_version() {
    let text = "module example.com/app\nrequire example.com/lib v1.2.0\nreplace example.com/lib v1.0.0 => example.com/lib v1.3.0\n";
    let file = parse_mod(text).expect("parses");
    assert_eq!(target_for(&file, "example.com/lib", "v1.2.0"), None);
    assert_eq!(
        target_for(&file, "example.com/lib", "v1.0.0"),
        Some(&upstream("example.com/lib", "v1.3.0"))
    );
    let local =
        parse_mod("module a\nreplace example.com/lib v1.0.0 => ../local\n").expect("parses");
    assert_eq!(target_for(&local, "example.com/lib", "v1.2.0"), None);
    assert_eq!(
        target_for(&local, "example.com/lib", "v1.0.0"),
        Some(&GoReplacementTarget::FilePath("../local".to_owned()))
    );
}

#[test]
fn replacement_for_prefers_specific_version_over_wildcard_in_any_order() {
    for text in [
        "module a\nreplace (\n\texample.com/lib => example.com/wild v1.0.0\n\texample.com/lib v1.2.0 => example.com/exact v2.0.0 // pin\n)\n",
        "module a\nreplace (\n\texample.com/lib v1.2.0 => example.com/exact v2.0.0\n\t// comment\n\texample.com/lib => example.com/wild v1.0.0\n)\n",
    ] {
        let file = parse_mod(text).expect("parses");
        assert_eq!(
            target_for(&file, "example.com/lib", "v1.2.0"),
            Some(&upstream("example.com/exact", "v2.0.0"))
        );
        assert_eq!(
            target_for(&file, "example.com/lib", "v1.5.0"),
            Some(&upstream("example.com/wild", "v1.0.0"))
        );
        assert_eq!(target_for(&file, "example.com/other", "v1.2.0"), None);
    }
}

#[test]
fn duplicate_replacements_for_one_module_version_are_errors() {
    let wildcard =
        "module a\nreplace example.com/lib => ../one\nreplace example.com/lib => ../two\n";
    assert!(parse_mod(wildcard)
        .expect_err("rejects")
        .contains("multiple replacements"));
    let exact = "module a\nreplace (\n\texample.com/lib v1.0.0 => ../one\n\texample.com/lib v1.0.0 => ../two\n)\n";
    assert!(parse_mod(exact).is_err());
    let mixed =
        "module a\nreplace example.com/lib v1.0.0 => ../one\nreplace example.com/lib => ../two\n";
    assert!(parse_mod(mixed).is_ok());
}
