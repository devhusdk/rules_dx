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
        "schema_version = 1\nprofile = \"strict\"\n[families.rust]\nlint = [\"typos\"]\n[families.go]\ndisabled = true\n",
    )
    .unwrap();
    let selection = effective(Some(&config)).unwrap();
    assert_eq!(selection.families["rust"].lint, ["typos"]);
    assert_eq!(selection.families["rust"].format, ["rustfmt"]);
    assert_eq!(selection.families["rust"].profile.as_str(), "strict");
    assert!(!selection.families.contains_key("go"));
}

#[test]
fn an_unknown_family_gets_a_hint() {
    let error = problem("schema_version = 1\n[families.pyhton]\ndisabled = true\n");
    assert!(error.to_string().contains("unknown family `pyhton`"));
    assert!(error.to_string().contains("did you mean `python`?"));
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
        "schema_version = 1\n[families.go]\nprofile = \"strict\"\nlint = [\"cppcheck\"]\n[families.rust]\nlint = [\"cppcheck\"]\n",
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
        "schema_version = 1\n[families.go]\nprofile = \"strict\"\n[families.rust]\nprofile = \"standard\"\n",
    )
    .unwrap();
    assert!(effective(Some(&config)).is_ok());
}
