//! Docs parity checks that need the exec and adopt layers: notice codes and
//! umbrella phase routing named in docs/cli/commands.

use std::path::{Path, PathBuf};

use dx_plan::args::Command;

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn docs_dir() -> PathBuf {
    workspace_root().join("docs/cli/commands")
}

fn sections(page: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut heading = String::new();
    let mut body: Vec<&str> = Vec::new();
    for line in page.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            out.push((std::mem::take(&mut heading), body.join("\n")));
            heading = rest.trim().to_owned();
            body.clear();
            continue;
        }
        body.push(line);
    }
    out.push((heading, body.join("\n")));
    out
}

fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('`') else {
            break;
        };
        out.push(tail[..end].to_owned());
        rest = &tail[end + 1..];
    }
    out
}

#[test]
fn clean_page_names_every_clean_notice_code() {
    use crate::exec::common::{
        CODE_CLEAN_FAILED, CODE_CLEAN_PLANNED, CODE_CLEAN_PRESERVED_LEASED,
        CODE_CLEAN_PRESERVED_UNOBSERVED, CODE_CLEAN_PRUNED,
    };
    let page =
        std::fs::read_to_string(docs_dir().join("check-fix-clean.md")).expect("check-fix-clean.md");
    let (_, body) = sections(&page)
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
        let (_, body) = sections(&page)
            .into_iter()
            .find(|(heading, _)| heading == name)
            .unwrap_or_else(|| panic!("new-upgrade.md has a {name} section"));
        backticked(&body)
    };
    let init_files: Vec<String> = dx_adopt::plan_init_files("demo", dx_adopt::InitParts::ALL)
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

#[test]
fn init_page_lists_each_part_with_its_files() {
    let page = std::fs::read_to_string(docs_dir().join("init.md")).expect("init.md");
    for part in dx_adopt::InitPart::ALL {
        let opening = format!("| `{}` |", part.name());
        let row = page
            .lines()
            .find(|line| line.starts_with(&opening))
            .unwrap_or_else(|| panic!("init.md parts table has no `{}` row", part.name()));
        let named = backticked(row);
        for path in part.scaffold_paths() {
            assert!(
                named.iter().any(|token| token == path),
                "init.md `{}` row never names `{path}`",
                part.name()
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
    use dx_plan::plan::registry::spec;
    let phases: Vec<Command> = super::UMBRELLA_PHASES
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

fn fenced_blocks(page: &str, fence: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        if !open && line.trim() == fence {
            open = true;
            current.clear();
            continue;
        }
        if open && line.trim() == "```" {
            open = false;
            blocks.push(std::mem::take(&mut current));
            continue;
        }
        if open {
            current.push(line.to_owned());
        }
    }
    assert!(!open, "unterminated {fence} block");
    blocks
}

fn usage_commands(page: &str) -> Vec<String> {
    fenced_blocks(page, "```text")
        .into_iter()
        .flatten()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some("dx")).then(|| words.next().map(str::to_owned))?
        })
        .collect()
}

fn command_pages() -> Vec<(String, String)> {
    let mut pages: Vec<(String, String)> = std::fs::read_dir(docs_dir())
        .expect("docs/cli/commands")
        .map(|entry| entry.expect("docs entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let name = path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            let page = std::fs::read_to_string(&path).expect("docs page");
            (name, page)
        })
        .collect();
    pages.sort();
    pages
}

#[test]
fn every_notice_code_is_named_on_its_command_page() {
    use super::notice_codes::{notice_codes, SHARED_NOTICE_CODES};
    use clap::ValueEnum;
    let pages = command_pages();
    for command in Command::value_variants() {
        let name = command.name();
        let own: Vec<&(String, String)> = pages
            .iter()
            .filter(|(_, page)| usage_commands(page).iter().any(|word| word == name))
            .collect();
        assert!(
            !own.is_empty(),
            "no docs/cli/commands page has a `dx {name}` usage line"
        );
        for code in notice_codes(*command) {
            assert!(
                own.iter()
                    .any(|(_, page)| backticked(page).iter().any(|token| token == code)),
                "dx {name} emits notice `{code}` but none of {:?} names it",
                own.iter().map(|(page, _)| page).collect::<Vec<_>>()
            );
        }
    }
    let readme = std::fs::read_to_string(docs_dir().join("README.md")).expect("README.md");
    let tokens = backticked(&readme);
    for code in SHARED_NOTICE_CODES {
        assert!(
            tokens.iter().any(|token| token == code),
            "docs/cli/commands/README.md never names shared notice `{code}`"
        );
    }
}

fn literal_notice_codes(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let cut = lines
        .iter()
        .enumerate()
        .find(|(index, line)| {
            line.trim() == "#[cfg(test)]"
                && lines[index + 1..]
                    .iter()
                    .find(|next| !next.trim().is_empty())
                    .is_some_and(|next| next.trim_start().starts_with("mod "))
        })
        .map_or(lines.len(), |(index, _)| index);
    let code = lines[..cut].join("\n");
    let mut found = Vec::new();
    for call in ["NoticeEvent::info(", "NoticeEvent::warning("] {
        for (at, _) in code.match_indices(call) {
            let argument = code[at + call.len()..].trim_start();
            if argument.starts_with('"')
                || argument.starts_with("&format!")
                || argument.starts_with("format!")
            {
                found.push(format!(
                    "{call}{}",
                    argument.lines().next().unwrap_or_default()
                ));
            }
        }
    }
    for (at, _) in code.match_indices("notice_code:") {
        let value = code[at + "notice_code:".len()..].trim_start();
        if value.starts_with('"') {
            found.push(format!(
                "notice_code: {}",
                value.lines().next().unwrap_or_default()
            ));
        }
    }
    found
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source dir") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn notice_codes_are_never_literals() {
    let mut sources = Vec::new();
    rust_sources(&workspace_root().join("cli/cli/src"), &mut sources);
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("exec/notice_codes.rs")),
        "cli/cli/src sources are not in the test data"
    );
    for path in sources {
        let file = path
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        if file.ends_with("_tests.rs") || file == "test_support.rs" {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("source");
        let found = literal_notice_codes(&source);
        assert!(
            found.is_empty(),
            "{}: notice codes must be consts in exec/notice_codes.rs: {found:?}",
            path.display()
        );
    }
}

#[test]
fn literal_scanner_catches_multi_line_literals_and_skips_test_modules() {
    let source = "fn a() {\n    NoticeEvent::warning(\n        \"oops\",\n        m,\n    );\n    \
                  NoticeEvent::info(CODE_OK, m);\n    NoticeEvent::info(&format!(\"x_{y}\"), m);\n}\n\
                  const V: EditVerb = EditVerb { notice_code: \"bad\" };\n\
                  #[cfg(test)]\nconst KEPT: &str = \"\";\nfn b() { NoticeEvent::info(\"after\", m); }\n\
                  #[cfg(test)]\nmod tests {\n    fn t() { NoticeEvent::info(\"test_only\", m); }\n}\n";
    let found = literal_notice_codes(source);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        found.iter().any(|hit| hit.contains("warning(\"oops\"")),
        "{found:?}"
    );
    assert!(found.iter().any(|hit| hit.contains("format!")), "{found:?}");
    assert!(
        found.iter().any(|hit| hit.contains("\"after\"")),
        "{found:?}"
    );
    assert!(found.iter().any(|hit| hit.contains("\"bad\"")), "{found:?}");
    assert!(
        !found.iter().any(|hit| hit.contains("test_only")),
        "{found:?}"
    );
}

#[test]
fn notice_code_consts_are_registered() {
    use super::notice_codes::{notice_codes, SHARED_NOTICE_CODES};
    use clap::ValueEnum;
    let registered: Vec<&str> = Command::value_variants()
        .iter()
        .flat_map(|command| notice_codes(*command).iter().copied())
        .chain(SHARED_NOTICE_CODES.iter().copied())
        .collect();
    let consts: Vec<&str> = include_str!("notice_codes.rs")
        .lines()
        .filter(|line| line.trim_start().starts_with("pub(crate) const CODE_"))
        .filter_map(|line| line.split('"').nth(1))
        .collect();
    assert!(
        !consts.is_empty(),
        "no CODE_ consts found in notice_codes.rs"
    );
    for code in consts {
        assert!(
            registered.contains(&code),
            "notice code `{code}` is in no command's list"
        );
    }
}
