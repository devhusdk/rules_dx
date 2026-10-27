use dx_cli_core::args::Command;
use dx_cli_core::plan::spec;
use dx_testing::cli::{backticked, markdown_sections};
use std::path::PathBuf;

fn docs_dir() -> PathBuf {
    dx_testing::runfiles_root().join("docs/cli/commands")
}

#[test]
fn init_page_shows_the_agents_block() {
    let page = std::fs::read_to_string(docs_dir().join("init.md")).expect("init.md");
    assert!(
        page.contains(&crate::adopt::agents::block()),
        "docs/cli/commands/init.md must show the block dx init writes to AGENTS.md"
    );
}

#[test]
fn clean_page_names_every_clean_notice_code() {
    use crate::exec::common::{
        CODE_CLEAN_FAILED, CODE_CLEAN_PLANNED, CODE_CLEAN_PRESERVED_LEASED,
        CODE_CLEAN_PRESERVED_UNOBSERVED, CODE_CLEAN_PRUNED,
    };
    let page =
        std::fs::read_to_string(docs_dir().join("check-fix-clean.md")).expect("check-fix-clean.md");
    let (_, body) = markdown_sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx clean`")
        .expect("check-fix-clean.md has a `dx clean` section");
    let tokens = backticked(&body);
    for name in [
        CODE_CLEAN_PLANNED,
        CODE_CLEAN_PRUNED,
        CODE_CLEAN_PRESERVED_LEASED,
        CODE_CLEAN_PRESERVED_UNOBSERVED,
        CODE_CLEAN_FAILED,
        "check",
        "default",
    ] {
        assert!(
            tokens.iter().any(|token| token == name),
            "check-fix-clean.md `dx clean` section never names `{name}`"
        );
    }
}

#[test]
fn new_upgrade_page_names_codes_and_starter_files() {
    use crate::adopt::{CODE_NEW_FAILED, CODE_STATUS_PIN_MISMATCH, CODE_UPGRADE_FAILED};
    let page = std::fs::read_to_string(docs_dir().join("new-upgrade.md")).expect("new-upgrade.md");
    let section = |name: &str| -> Vec<String> {
        let (_, body) = markdown_sections(&page)
            .into_iter()
            .find(|(heading, _)| heading == name)
            .unwrap_or_else(|| panic!("new-upgrade.md has a {name} section"));
        backticked(&body)
    };
    let init_files: Vec<String> = dx_adopt::plan_init_files("demo")
        .expect("demo is a valid module name")
        .into_iter()
        .map(|file| file.path)
        .collect();
    let mut new_names = vec![CODE_NEW_FAILED.to_owned()];
    new_names.extend(init_files);
    for (heading, names) in [
        ("`dx new`", new_names),
        ("`dx upgrade`", vec![CODE_UPGRADE_FAILED.to_owned()]),
        (
            "`dx rollback`",
            vec![CODE_STATUS_PIN_MISMATCH.to_owned(), "pin".to_owned()],
        ),
    ] {
        let tokens = section(heading);
        for name in names {
            assert!(
                tokens.iter().any(|token| *token == name),
                "new-upgrade.md {heading} section never names `{name}`"
            );
        }
    }
}

fn sentence_names(page: &str, opening: &str) -> Vec<String> {
    let flat = page.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = flat
        .find(opening)
        .unwrap_or_else(|| panic!("check-fix-clean.md has no sentence starting {opening:?}"));
    let rest = &flat[start + opening.len()..];
    let end = rest.find(". ").unwrap_or(rest.len());
    let mut names = backticked(&rest[..end]);
    names.sort();
    names
}

#[test]
fn umbrella_phase_flag_routing_matches_docs() {
    let phases: Vec<Command> = crate::exec::UMBRELLA_PHASES
        .into_iter()
        .chain([Command::Test])
        .collect();
    let mut fail_on: Vec<String> = phases
        .iter()
        .filter(|phase| phase.supports_fail_on())
        .map(|phase| phase.name().to_owned())
        .collect();
    fail_on.sort();
    let mut sarif: Vec<String> = phases
        .iter()
        .filter(|phase| **phase != Command::Test && spec(**phase).accepts_report("sarif"))
        .map(|phase| phase.name().to_owned())
        .collect();
    sarif.sort();
    let page =
        std::fs::read_to_string(docs_dir().join("check-fix-clean.md")).expect("check-fix-clean.md");
    assert_eq!(sentence_names(&page, "`--fail-on` applies to"), fail_on);
    assert_eq!(
        sentence_names(&page, "`--report sarif=<dest>` merges"),
        sarif
    );
}
