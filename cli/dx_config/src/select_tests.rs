use crate::config::parse;
use crate::curated_defaults_gen::{CROSS_FAMILY_TOOLS, FAMILIES, TOOLS};
use crate::select::{SelectionProblem, effective};

fn problem(text: &str) -> SelectionProblem {
    effective(Some(&parse(text).unwrap())).unwrap_err()
}

#[test]
fn no_file_is_the_curated_standard_tier() {
    let selection = effective(None).unwrap();
    assert_eq!(selection.families.len(), FAMILIES.len());
    assert_eq!(selection.families["rust"].format, ["rustfmt"]);
    assert_eq!(selection.families["rust"].profile.as_str(), "standard");
}

#[test]
fn curated_tools_are_known_tools() {
    for family in FAMILIES {
        for tool in family
            .lint
            .iter()
            .chain(family.format)
            .chain(family.typecheck)
            .chain(family.audit)
        {
            assert!(TOOLS.contains(tool), "{} uses unknown {tool}", family.id);
        }
    }
    for tool in CROSS_FAMILY_TOOLS {
        assert!(TOOLS.contains(tool), "{tool}");
    }
}

#[test]
fn a_family_override_replaces_one_capability() {
    let config = parse(
        "schema_version = 1\ntier = \"strict\"\n[families.rust]\nlint = [\"typos\"]\n[families.go]\ndisabled = true\n",
    )
    .unwrap();
    let selection = effective(Some(&config)).unwrap();
    assert_eq!(selection.families["rust"].lint, ["typos"]);
    assert_eq!(selection.families["rust"].format, ["rustfmt"]);
    assert_eq!(selection.families["rust"].profile.as_str(), "strict");
    assert!(!selection.families.contains_key("go"));
    assert_eq!(selection.disabled, ["go".to_owned()].into());
}

#[test]
fn an_unknown_family_gets_a_hint() {
    let error = problem("schema_version = 1\n[families.pyhton]\ndisabled = true\n");
    assert!(error.to_string().contains("unknown family `pyhton`"));
    assert!(error.to_string().contains("did you mean `python`?"));
}

#[test]
fn an_opt_in_family_runs_only_the_tools_it_names() {
    let config = parse(
        "schema_version = 1\n[families.lua]\nformat = [\"stylua\"]\n[families.flatbuffers]\nlint = [\"flatc\"]\n",
    )
    .unwrap();
    let selection = effective(Some(&config)).unwrap();
    assert_eq!(selection.families["lua"].format, ["stylua"]);
    assert!(selection.families["lua"].lint.is_empty());
    assert_eq!(selection.families["lua"].profile.as_str(), "standard");
    assert_eq!(selection.families["flatbuffers"].lint, ["flatc"]);
    let unlisted = effective(None).unwrap();
    assert!(!unlisted.families.contains_key("lua"));
}

#[test]
fn a_misspelled_opt_in_family_gets_a_hint() {
    let error = problem("schema_version = 1\n[families.luaa]\nformat = [\"stylua\"]\n");
    assert!(error.to_string().contains("did you mean `lua`?"));
}

#[test]
fn an_unknown_tool_gets_a_hint() {
    let error = problem("schema_version = 1\n[families.rust]\nlint = [\"clipy\"]\n");
    assert!(error.to_string().contains("unknown tool `clipy`"));
    assert!(error.to_string().contains("did you mean `clippy`?"));
    let error = problem("schema_version = 1\n[families.rust]\nlint = [\"zzzzzzzzzz\"]\n");
    assert!(!error.to_string().contains("did you mean"));
}

#[test]
fn two_profiles_on_one_shared_tool_conflict() {
    let error = problem(
        "schema_version = 1\n[families.go]\ntier = \"strict\"\nlint = [\"cppcheck\"]\n[families.rust]\nlint = [\"cppcheck\"]\n",
    );
    let text = error.to_string();
    assert!(text.contains("`cppcheck`"), "{text}");
    assert!(
        text.contains("`go` (strict)") && text.contains("`rust` (standard)"),
        "{text}"
    );
}

#[test]
fn cross_family_tools_follow_the_root_profile() {
    let config = parse(
        "schema_version = 1\n[families.go]\ntier = \"strict\"\n[families.rust]\ntier = \"standard\"\n",
    )
    .unwrap();
    assert!(effective(Some(&config)).is_ok());
}

#[test]
fn two_owners_of_one_concern_are_rejected() {
    let error =
        problem("schema_version = 1\n[families.javascript]\nlint = [\"eslint\", \"biome\"]\n");
    let text = error.to_string();
    assert!(
        text.contains("`eslint`") && text.contains("`biome`"),
        "{text}"
    );
    assert!(text.contains("`lint`"), "{text}");
}

#[test]
fn skip_tools_drops_a_tool_everywhere() {
    let config = parse("schema_version = 1\nskip_tools = [\"typos\"]\n").unwrap();
    let selection = effective(Some(&config)).unwrap();
    assert!(
        selection
            .families
            .values()
            .all(|family| !family.lint.iter().any(|tool| tool == "typos"))
    );
    assert_eq!(selection.families["rust"].format, ["rustfmt"]);
}

#[test]
fn plus_and_minus_edit_the_curated_list() {
    let curated = effective(None).unwrap().families["cc"].lint.clone();
    assert!(curated.iter().any(|tool| tool == "typos"));
    let config = parse(
        "schema_version = 1\n[families.cc]\nlint = [\"+cppcheck\", \"-clang_tidy\"]\n",
    )
    .unwrap();
    let lint = effective(Some(&config)).unwrap().families["cc"].lint.clone();
    let mut expected: Vec<String> = curated.into_iter().filter(|tool| tool != "clang_tidy").collect();
    expected.push("cppcheck".to_owned());
    assert_eq!(lint, expected);
}

#[test]
fn mixed_edits_and_unknown_skips_are_rejected() {
    assert!(matches!(
        problem("schema_version = 1\n[families.cc]\nlint = [\"+cppcheck\", \"typos\"]\n"),
        SelectionProblem::MixedEdit { .. }
    ));
    assert!(matches!(
        problem("schema_version = 1\nskip_tools = [\"nope\"]\n"),
        SelectionProblem::UnknownTool { .. }
    ));
}
