use std::io::Write;
use std::path::Path;

use super::command::Command;
use super::completion::COMPLETION_SHELLS;
use super::grammar::VALUE_OPTIONS;

pub const COMPLETE_SUBCOMMAND: &str = "__complete";

pub const DYNAMIC_MARKER: &str = "dx dynamic candidates";

pub use dx_adopt::{HOOK_TRIGGERS, HOOK_VERBS};

const MAX_LABEL_CANDIDATES: usize = 100;

const MAX_PACKAGE_DIRS: usize = 200;

const MAX_VISITED_DIRS: usize = 2000;

fn prefixed(names: &[&str], current: &str) -> Vec<String> {
    let mut out: Vec<String> = names
        .iter()
        .filter(|name| name.starts_with(current))
        .map(ToString::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn update_set_hints(current: &str) -> Vec<String> {
    let names: Vec<&str> = dx_update::sets::SetId::ALL
        .iter()
        .map(|id| id.name())
        .collect();
    prefixed(&names, current)
}

pub fn slot_candidates(command: Command, prior: &[String], current: &str) -> (Vec<String>, bool) {
    use super::command::FirstSlot;
    let meta = command.meta();
    let fixed = if prior.is_empty() {
        match meta.first_slot {
            FirstSlot::None => Vec::new(),
            FirstSlot::UpdateSets => update_set_hints(current),
            FirstSlot::NewLanguages => prefixed(dx_adopt::SUPPORTED_NEW_LANGUAGES, current),
            FirstSlot::HookVerbs => prefixed(HOOK_VERBS, current),
            FirstSlot::WatchTasks => prefixed(dx_adopt::WATCHABLE_COMMANDS, current),
            FirstSlot::CompletionShells => prefixed(COMPLETION_SHELLS, current),
        }
    } else if prior.len() == 1 && prior[0] == "run" && meta.hook_triggers_on_run {
        prefixed(HOOK_TRIGGERS, current)
    } else {
        Vec::new()
    };
    (fixed, command.allows_labels(prior.len()))
}

pub fn completes_labels(command: Command) -> bool {
    let empty: Vec<String> = Vec::new();
    let one = vec![String::from("scope")];
    let two = vec![String::from("a"), String::from("b")];
    slot_candidates(command, &empty, "").1
        || slot_candidates(command, &one, "").1
        || slot_candidates(command, &two, "").1
}

pub fn top_candidates(current: &str) -> Vec<String> {
    use clap::ValueEnum;
    let names: Vec<&str> = Command::value_variants()
        .iter()
        .map(|command| command.name())
        .collect();
    prefixed(&names, current)
}

fn label_hint(current: &str) -> Vec<String> {
    if current.starts_with('@') {
        Vec::new()
    } else if "//...".starts_with(current) {
        vec!["//...".to_owned()]
    } else {
        Vec::new()
    }
}

pub fn label_candidates_from_workspace(
    workspace: &Path,
    current: &str,
    limit: usize,
) -> Vec<String> {
    if current.starts_with('@') {
        return Vec::new();
    }
    if !current.is_empty() && !current.starts_with("//") {
        return Vec::new();
    }
    let mut patterns: Vec<String> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![workspace.to_path_buf()];
    let mut visited: usize = 0;
    while let Some(dir) = stack.pop() {
        if visited >= MAX_VISITED_DIRS || patterns.len() >= MAX_PACKAGE_DIRS {
            break;
        }
        visited += 1;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut has_build = false;
        let mut subdirs: Vec<std::path::PathBuf> = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with("bazel-") {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                subdirs.push(entry.path());
            } else if name == "BUILD.bazel" || name == "BUILD" {
                has_build = true;
            }
        }
        if has_build {
            match dir.strip_prefix(workspace) {
                Ok(rel) if rel.as_os_str().is_empty() => {}
                Ok(rel) => {
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    let pattern = format!("//{rel}/...");
                    if pattern.starts_with(current) {
                        patterns.push(pattern);
                    }
                }
                Err(_) => {}
            }
        }
        subdirs.sort();
        for sub in subdirs.into_iter().rev() {
            stack.push(sub);
        }
    }
    patterns.sort();
    patterns.dedup();
    patterns.truncate(limit);
    patterns
}

fn bare_words(prior_all: &[String]) -> Vec<String> {
    let mut bare: Vec<String> = Vec::new();
    let mut index = 0;
    while index < prior_all.len() {
        let word = prior_all[index].as_str();
        if word == "--" {
            break;
        }
        if word.starts_with('-') {
            let name = word.split_once('=').map_or(word, |(name, _)| name);
            if !word.contains('=') && VALUE_OPTIONS.contains(&name) {
                match prior_all.get(index + 1) {
                    Some(next) if !next.starts_with("--") && next != "--" => index += 2,
                    _ => index += 1,
                }
                continue;
            }
            index += 1;
            continue;
        }
        bare.push(word.to_owned());
        index += 1;
    }
    bare
}

pub fn run_complete<S: AsRef<std::ffi::OsStr>>(
    words: &[S],
    cwd: &Path,
    out: &mut dyn Write,
) -> i32 {
    let owned: Vec<String> = words
        .iter()
        .map(|word| word.as_ref().to_string_lossy().into_owned())
        .collect();
    let current: &str = owned.last().map(String::as_str).unwrap_or("");
    let prior_all: &[String] = if owned.is_empty() {
        &[]
    } else {
        &owned[..owned.len() - 1]
    };
    let bare = bare_words(prior_all);
    let command = bare.first().and_then(|word| Command::parse(word));
    let mut candidates: Vec<String> = match command {
        None => top_candidates(current),
        Some(cmd) => {
            let scopes: Vec<String> = bare[1..].to_vec();
            let (fixed, allow_labels) = slot_candidates(cmd, &scopes, current);
            let mut merged = fixed;
            if allow_labels {
                merged.extend(label_hint(current));
                let start = dx_process::workspace_start(cwd);
                let workspace = dx_process::discover_real(&start, None).unwrap_or(start);
                merged.extend(label_candidates_from_workspace(
                    &workspace,
                    current,
                    MAX_LABEL_CANDIDATES,
                ));
            }
            merged.sort();
            merged.dedup();
            merged
        }
    };
    candidates.sort();
    candidates.dedup();
    for candidate in &candidates {
        if let Err(error) = writeln!(out, "{candidate}") {
            return dx_process::stdout_io_code(&error);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn complete_subcommand_name_is_stable() {
        assert_eq!(COMPLETE_SUBCOMMAND, "__complete");
        assert_eq!(DYNAMIC_MARKER, "dx dynamic candidates");
    }

    #[test]
    fn fixed_tables_match_their_single_sources() {
        assert_eq!(HOOK_VERBS, &["install", "uninstall", "status", "run"]);
        assert_eq!(HOOK_TRIGGERS, &["pre-commit", "pre-push"]);
        for trigger in HOOK_TRIGGERS {
            assert!(
                dx_adopt::is_hook_trigger(trigger),
                "{trigger} must stay a hook trigger"
            );
        }
        assert!(!dx_adopt::is_hook_trigger("pre-merge"));
        assert_eq!(COMPLETION_SHELLS, &["bash", "zsh", "fish", "powershell"]);
        let mut sets: Vec<&str> = dx_update::sets::SetId::ALL
            .iter()
            .map(|id| id.name())
            .collect();
        sets.sort_unstable();
        assert_eq!(
            sets,
            vec![
                "cargo",
                "go",
                "maven",
                "npm",
                "npm-adopt",
                "npm-adopt-polyglot",
                "npm-tools",
                "nuget",
                "uv",
                "uv-adopt",
                "uv-adopt-polyglot",
                "uv-tools",
            ]
        );
    }

    #[test]
    fn slot_candidates_cover_every_command_without_drift() {
        let empty: Vec<String> = Vec::new();
        for command in Command::value_variants() {
            let (fixed, labels) = slot_candidates(*command, &empty, "");
            let (filtered, _) = slot_candidates(*command, &empty, "zzz-no-match-zzz");
            assert!(
                filtered.is_empty(),
                "{command:?} must prefix-filter fixed candidates"
            );
            match command {
                Command::Security | Command::License => {
                    assert!(fixed.is_empty());
                    assert!(labels);
                }
                Command::Watch => {
                    let mut want: Vec<String> = dx_adopt::WATCHABLE_COMMANDS
                        .iter()
                        .map(ToString::to_string)
                        .collect();
                    want.sort();
                    assert_eq!(fixed, want);
                    assert!(!labels);
                }
                Command::Hooks => {
                    assert_eq!(fixed, vec!["install", "run", "status", "uninstall"]);
                    assert!(!labels);
                }
                Command::New => {
                    let mut want: Vec<String> = dx_adopt::SUPPORTED_NEW_LANGUAGES
                        .iter()
                        .map(ToString::to_string)
                        .collect();
                    want.sort();
                    assert_eq!(fixed, want);
                    assert!(!labels);
                }
                Command::Completion => {
                    assert_eq!(fixed, vec!["bash", "fish", "powershell", "zsh"]);
                    assert!(!labels);
                }
                Command::Update | Command::Bump => {
                    assert_eq!(
                        fixed,
                        vec![
                            "cargo",
                            "go",
                            "maven",
                            "npm",
                            "npm-adopt",
                            "npm-adopt-polyglot",
                            "npm-tools",
                            "nuget",
                            "uv",
                            "uv-adopt",
                            "uv-adopt-polyglot",
                            "uv-tools",
                        ]
                    );
                    assert_eq!(labels, *command == Command::Update);
                }
                Command::Clean
                | Command::Init
                | Command::Upgrade
                | Command::Status
                | Command::Version
                | Command::Bazel => {
                    assert!(fixed.is_empty(), "{command:?} takes no candidates");
                    assert!(!labels, "{command:?} takes no labels");
                }
                _ => {
                    assert!(fixed.is_empty(), "{command:?} takes no fixed tasks");
                    assert!(labels, "{command:?} must accept label scopes");
                }
            }
            assert_eq!(
                slot_candidates(*command, &empty, ""),
                slot_candidates(*command, &empty, ""),
                "{command:?} must be deterministic"
            );
        }
        assert_eq!(Command::value_variants().len(), 33);
    }

    #[test]
    fn later_slots_follow_per_command_shapes() {
        let run = vec![String::from("run")];
        let (fixed, labels) = slot_candidates(Command::Hooks, &run, "");
        assert_eq!(fixed, vec!["pre-commit", "pre-push"]);
        assert!(!labels);
        let (fixed, _) = slot_candidates(Command::Hooks, &run, "pre-c");
        assert_eq!(fixed, vec!["pre-commit"]);
        let install = vec![String::from("install")];
        assert_eq!(
            slot_candidates(Command::Hooks, &install, ""),
            (Vec::new(), false)
        );
        let task = vec![String::from("test")];
        assert_eq!(
            slot_candidates(Command::Watch, &task, ""),
            (Vec::new(), true)
        );
        let one = vec![String::from("//a:one")];
        for command in [
            Command::Codegen,
            Command::Env,
            Command::Setup,
            Command::Bump,
            Command::New,
            Command::Completion,
        ] {
            assert_eq!(
                slot_candidates(command, &one, ""),
                (Vec::new(), false),
                "{command:?} must close after its slot"
            );
        }
        for command in [
            Command::Build,
            Command::Owners,
            Command::Security,
            Command::License,
            Command::Update,
            Command::Why,
        ] {
            assert!(
                slot_candidates(command, &one, "").1,
                "{command:?} must stay open for more scopes"
            );
        }
        let two = vec![String::from("a"), String::from("b")];
        assert_eq!(
            slot_candidates(Command::Why, &two, ""),
            (Vec::new(), false),
            "Why must close after its pair"
        );
    }

    #[test]
    fn top_candidates_list_the_command_table() {
        let all = top_candidates("");
        assert_eq!(all.len(), 33);
        assert!(all.contains(&"build".to_owned()));
        assert!(all.contains(&"completion".to_owned()));
        assert_eq!(top_candidates("b"), vec!["bazel", "build", "bump"]);
        assert!(top_candidates("zzz-no-match-zzz").is_empty());
    }

    #[test]
    fn label_scan_lists_workspace_packages_and_skips_non_packages() {
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path();
        for dir in ["", "pkg/a", "pkg/b", ".hidden", "bazel-out"] {
            std::fs::create_dir_all(root.join(dir)).expect("mkdir");
        }
        for marker in [
            "BUILD.bazel",
            "pkg/a/BUILD.bazel",
            "pkg/b/BUILD",
            ".hidden/BUILD.bazel",
            "bazel-out/BUILD.bazel",
        ] {
            std::fs::write(root.join(marker), "").expect("marker");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("pkg/a"), root.join("linked")).expect("symlink");
        std::fs::write(root.join("linked/BUILD.bazel"), "").expect("marker");
        let mut got = label_candidates_from_workspace(root, "", 100);
        got.sort();
        assert_eq!(got, vec!["//pkg/a/...", "//pkg/b/..."]);
        assert_eq!(
            label_candidates_from_workspace(root, "//pkg/a", 100),
            vec!["//pkg/a/..."]
        );
        assert!(label_candidates_from_workspace(root, "@repo//...", 100).is_empty());
        assert!(label_candidates_from_workspace(root, "src/main.rs", 100).is_empty());
        assert_eq!(
            label_candidates_from_workspace(root, "//", 1),
            vec!["//pkg/a/..."]
        );
    }

    #[test]
    fn run_complete_lists_tasks_labels_and_commands_and_fails_open() {
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("pkg")).expect("mkdir");
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("marker");
        let run = |words: &[&str]| {
            let owned: Vec<String> = words.iter().map(ToString::to_string).collect();
            let mut out = Vec::new();
            let code = run_complete(&owned, root, &mut out);
            assert_eq!(code, 0);
            String::from_utf8(out).expect("out")
        };
        let text = run(&["watch", ""]);
        for task in dx_adopt::WATCHABLE_COMMANDS {
            assert!(text.contains(task), "watch must offer {task}:\n{text}");
        }
        let text = run(&["build", ""]);
        assert!(text.contains("//..."), "build must offer //...:\n{text}");
        assert!(
            text.contains("//pkg/..."),
            "build must offer //pkg/...:\n{text}"
        );
        let text = run(&["build", "//pkg"]);
        assert!(
            text.contains("//pkg/..."),
            "build must filter //pkg:\n{text}"
        );
        assert!(
            !text.contains("//other"),
            "build must not offer //other:\n{text}"
        );
        let text = run(&["--workspace", "/tmp", "build", ""]);
        assert!(
            text.contains("//..."),
            "flags must not shift slots:\n{text}"
        );
        let text = run(&["hooks", "run", ""]);
        assert!(
            text.contains("pre-commit"),
            "hooks run must offer triggers:\n{text}"
        );
        let text = run(&[""]);
        assert!(
            text.contains("build"),
            "bare slot must offer commands:\n{text}"
        );
        assert!(
            text.contains("completion"),
            "bare slot must offer commands:\n{text}"
        );
        let text = run(&["b"]);
        assert!(
            text.contains("build"),
            "partial command must filter:\n{text}"
        );
        assert!(
            !text.contains("test"),
            "partial command must filter:\n{text}"
        );
        let text = run(&["bogus-command-zzz", ""]);
        assert!(
            text.contains("build"),
            "unknown command words fail open toward commands:\n{text}"
        );
        let text = run(&["build", "@repo//..."]);
        assert_eq!(text, "", "external prefixes stay empty");
        let missing = run_complete(
            &["build".to_owned(), String::new()],
            Path::new("/nonexistent-zzz"),
            &mut Vec::new(),
        );
        assert_eq!(missing, 0);
    }
}
