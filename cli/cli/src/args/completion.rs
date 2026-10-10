use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use clap_complete::engine::{ArgValueCompleter, CompletionCandidate, ValueCompleter};

use super::command::{Command, FirstSlot, COMMANDS};
use super::grammar::VALUE_OPTIONS;
use super::ArgsError;

pub const COMPLETION_SHELLS: &[&str] = &["bash", "zsh", "fish", "powershell"];

/// The variable a shell sets to ask dx for candidates.
pub const COMPLETE_VAR: &str = "COMPLETE";

/// The binary name the rendered scripts call back into.
const COMPLETION_BIN: &str = "dx";

const MAX_LABEL_CANDIDATES: usize = 100;

const MAX_PACKAGE_DIRS: usize = 200;

const MAX_VISITED_DIRS: usize = 2000;

const QUERY_TIMEOUT: Duration = Duration::from_millis(1500);

/// Serve the shell request in COMPLETE, and report whether there was one.
pub fn try_complete() -> Result<bool, String> {
    clap_complete::CompleteEnv::with_factory(super::grammar::cli_command)
        .try_complete(std::env::args_os(), std::env::current_dir().ok().as_deref())
        .map_err(|error| error.to_string())
}

/// Give every command the completer for its own scope slot.
pub fn with_scope_completers(
    mut command: clap::Command,
    words: impl IntoIterator<Item = OsString>,
    start: &Path,
) -> clap::Command {
    let words: Vec<OsString> = words.into_iter().collect();
    let workspace = select_workspace(start, &words);
    let prior = prior_scopes(&words);
    for entry in COMMANDS.iter() {
        if entry.command.scope_policy() == "passthrough" {
            continue;
        }
        let completer = ArgValueCompleter::new(ScopeCompleter {
            command: entry.command,
            prior: prior.clone(),
            workspace: workspace.clone(),
        });
        let mut attached = false;
        command = command.mut_subcommand(entry.name, |sub| {
            sub.mut_args(|arg| {
                if arg.get_id() == "targets" {
                    attached = true;
                    arg.add(completer.clone())
                } else {
                    arg
                }
            })
        });
        assert!(attached, "dx {} must have a targets arg", entry.name);
    }
    command
}

/// Render the script one shell runs to ask dx for candidates.
pub fn render_completion(shell: &str) -> Result<String, ArgsError> {
    let unknown = || ArgsError::UnknownShell {
        shell: shell.to_owned(),
    };
    if !COMPLETION_SHELLS.contains(&shell) {
        return Err(unknown());
    }
    let shells = clap_complete::env::Shells::builtins();
    let completer = shells.completer(shell).ok_or_else(unknown)?;
    let mut script = Vec::new();
    completer
        .write_registration(
            COMPLETE_VAR,
            COMPLETION_BIN,
            COMPLETION_BIN,
            COMPLETION_BIN,
            &mut script,
        )
        .map_err(|_| unknown())?;
    String::from_utf8(script).map_err(|_| unknown())
}

/// Whether a rendered script calls back into dx for this shell.
pub fn registers_callback(script: &str, shell: &str) -> bool {
    !script.is_empty() && script.contains(COMPLETE_VAR) && script.contains(shell)
}

/// The scope words already typed, without the command name and the word in hand.
fn prior_scopes(words: &[OsString]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut index = match words.iter().position(|word| word == "--") {
        Some(separator) => separator + 1,
        None => 0,
    };
    while index < words.len() {
        let word = words[index].to_string_lossy().into_owned();
        index += 1;
        if word == "--" {
            break;
        }
        if word.starts_with('-') {
            let name = word.split_once('=').map_or(word.as_str(), |(name, _)| name);
            if !word.contains('=') && VALUE_OPTIONS.contains(&name) {
                index += 1;
            }
            continue;
        }
        out.push(word);
    }
    out.pop();
    match out.iter().position(|word| Command::parse(word).is_some()) {
        Some(command) => {
            out.drain(..=command);
        }
        None => out.clear(),
    }
    out
}

/// The candidates one command offers for its scope slot.
struct ScopeCompleter {
    command: Command,
    prior: Vec<String>,
    workspace: PathBuf,
}

impl ValueCompleter for ScopeCompleter {
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        self.complete_at(0, current)
    }

    fn complete_at(&self, _index: usize, current: &OsStr) -> Vec<CompletionCandidate> {
        let current = current.to_string_lossy();
        let (mut names, allows_labels) = slot_candidates(self.command, &self.prior, &current);
        if allows_labels {
            names.extend(label_hint(&current));
            names.extend(label_candidates_from_workspace(
                &self.workspace,
                &current,
                MAX_LABEL_CANDIDATES,
            ));
            names.extend(target_candidates(
                &self.workspace,
                &current,
                MAX_LABEL_CANDIDATES,
            ));
        }
        names.sort();
        names.dedup();
        names
            .into_iter()
            .map(CompletionCandidate::new)
            .collect::<Vec<CompletionCandidate>>()
    }
}

/// The explicit workspace the typed words select, if one is spelled.
fn workspace_override_from_words(words: &[OsString]) -> Option<PathBuf> {
    let mut trimmed: &[OsString] = words;
    if trimmed.first().is_some_and(|first| first == "--") {
        trimmed = &trimmed[1..];
        if trimmed.first().is_some_and(|first| first == "dx") {
            trimmed = &trimmed[1..];
        }
    } else if trimmed.first().is_some_and(|first| first == "dx") {
        trimmed = &trimmed[1..];
    }
    let flag = super::parser::early_workspace_flag(trimmed)?;
    if flag.is_empty() {
        None
    } else {
        Some(PathBuf::from(flag))
    }
}

/// The workspace completion reads: the flag override or the start directory.
fn select_workspace(start: &Path, words: &[OsString]) -> PathBuf {
    let override_dir = workspace_override_from_words(words);
    dx_process::discover_real(start, override_dir.as_deref())
        .unwrap_or_else(|_| start.to_path_buf())
}

/// Whether the directory opens another workspace.
fn is_workspace_root(dir: &Path) -> bool {
    dir.join("MODULE.bazel").is_file()
        || dir.join("WORKSPACE").is_file()
        || dir.join("WORKSPACE.bazel").is_file()
}

/// The ignored directory prefixes listed in the workspace `.bazelignore`.
fn read_bazelignore(workspace: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(workspace.join(".bazelignore")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("./").unwrap_or(line);
        let line = line.trim_end_matches('/');
        if line.is_empty() || line == "." {
            continue;
        }
        out.push(line.to_owned());
    }
    out.sort();
    out.dedup();
    out
}

/// Whether the workspace-relative path falls under an ignored prefix.
fn is_ignored(rel: &str, ignores: &[String]) -> bool {
    ignores
        .iter()
        .any(|prefix| rel == prefix || rel.starts_with(&format!("{prefix}/")))
}

/// The package the current word asks about, when it names a target.
fn package_of_current(current: &str) -> Option<String> {
    if current.starts_with('@') {
        return None;
    }
    if !current.starts_with("//") {
        return None;
    }
    let (package, _) = current.split_once(':')?;
    if package.is_empty() || package == "//..." {
        return None;
    }
    if package.contains("...") {
        return None;
    }
    Some(package.to_owned())
}

/// The bounded query that lists one package, never the whole workspace.
fn query_expression(package: &str) -> String {
    format!("kind(rule, {package}:*)")
}

/// The labels one bounded query returns, empty when it cannot.
fn bazel_query_labels(workspace: &Path, package: &str) -> Vec<String> {
    let expression = query_expression(package);
    let mut child = match std::process::Command::new("bazel")
        .args(["query", "--output=label", &expression])
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Vec::new(),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Vec::new();
                }
                let output = match child.wait_with_output() {
                    Ok(output) => output,
                    Err(_) => return Vec::new(),
                };
                let Ok(text) = String::from_utf8(output.stdout) else {
                    return Vec::new();
                };
                let mut out: Vec<String> = text
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToString::to_string)
                    .collect();
                out.sort();
                out.dedup();
                return out;
            }
            Ok(None) => {
                if start.elapsed() > QUERY_TIMEOUT {
                    let _ = child.kill();
                    return Vec::new();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return Vec::new(),
        }
    }
}

/// The target labels matching the current word through one bounded query.
fn target_candidates(workspace: &Path, current: &str, limit: usize) -> Vec<String> {
    target_candidates_with_query(workspace, current, limit, bazel_query_labels)
}

/// The target labels matching the current word through the given lookup.
fn target_candidates_with_query(
    workspace: &Path,
    current: &str,
    limit: usize,
    query: impl Fn(&Path, &str) -> Vec<String>,
) -> Vec<String> {
    let Some(package) = package_of_current(current) else {
        return Vec::new();
    };
    let mut out: Vec<String> = query(workspace, &package)
        .into_iter()
        .filter(|label| label.starts_with(current))
        .collect();
    out.sort();
    out.dedup();
    out.truncate(limit);
    out
}

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

fn bump_set_hints(current: &str) -> Vec<String> {
    prefixed(&dx_bump::BumpSet::names(), current)
}

fn slot_candidates(command: Command, prior: &[String], current: &str) -> (Vec<String>, bool) {
    let meta = command.meta();
    let fixed = if prior.is_empty() {
        match meta.first_slot {
            FirstSlot::None => Vec::new(),
            FirstSlot::UpdateSets => update_set_hints(current),
            FirstSlot::BumpSets => bump_set_hints(current),
            FirstSlot::NewLanguages => prefixed(dx_adopt::SUPPORTED_NEW_LANGUAGES, current),
            FirstSlot::HookVerbs => prefixed(dx_adopt::HOOK_VERBS, current),
            FirstSlot::WatchTasks => prefixed(dx_adopt::WATCHABLE_COMMANDS, current),
            FirstSlot::CompletionShells => prefixed(COMPLETION_SHELLS, current),
        }
    } else if prior.len() == 1 && prior[0] == "run" && meta.hook_triggers_on_run {
        prefixed(dx_adopt::HOOK_TRIGGERS, current)
    } else {
        Vec::new()
    };
    (fixed, command.allows_labels(prior.len()))
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

fn label_candidates_from_workspace(workspace: &Path, current: &str, limit: usize) -> Vec<String> {
    if current.starts_with('@') {
        return Vec::new();
    }
    if !current.is_empty() && !current.starts_with("//") {
        return Vec::new();
    }
    if current.contains(':') {
        return Vec::new();
    }
    let ignores = read_bazelignore(workspace);
    let mut patterns: Vec<String> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![workspace.to_path_buf()];
    let mut visited: usize = 0;
    while let Some(dir) = stack.pop() {
        if visited >= MAX_VISITED_DIRS || patterns.len() >= MAX_PACKAGE_DIRS {
            break;
        }
        visited += 1;
        if dir != workspace && is_workspace_root(&dir) {
            continue;
        }
        if dir != workspace {
            if let Ok(rel) = dir.strip_prefix(workspace) {
                let rel = dx_path::posix(rel);
                if !rel.is_empty() && is_ignored(&rel, &ignores) {
                    continue;
                }
            }
        }
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
                    let rel = dx_path::posix(rel);
                    if is_ignored(&rel, &ignores) {
                        continue;
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, ValueEnum};

    fn complete(argv: &[&str], workspace: &Path) -> Vec<String> {
        let words: Vec<OsString> = argv.iter().map(OsString::from).collect();
        complete_with_words(&words, words[1..].to_vec(), workspace)
    }

    /// Complete the way a shell does: the typed words sit behind a `--` separator.
    fn complete_from_a_shell(argv: &[&str], workspace: &Path) -> Vec<String> {
        let words: Vec<OsString> = argv.iter().map(OsString::from).collect();
        let mut passed = vec![OsString::from("--")];
        passed.extend(words.iter().cloned());
        complete_with_words(&words, passed, workspace)
    }

    fn complete_with_words(
        words: &[OsString],
        passed: Vec<OsString>,
        workspace: &Path,
    ) -> Vec<String> {
        let mut command =
            with_scope_completers(super::super::grammar::Cli::command(), passed, workspace);
        let index = words.len() - 1;
        clap_complete::engine::complete(&mut command, words.to_vec(), index, Some(workspace))
            .expect("complete")
            .iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect()
    }

    fn workspace_with_package() -> tempfile::TempDir {
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("pkg")).expect("mkdir");
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("marker");
        scratch
    }

    #[test]
    fn fixed_tables_match_their_single_sources() {
        assert_eq!(
            dx_adopt::HOOK_VERBS,
            &["install", "uninstall", "status", "run"]
        );
        assert_eq!(dx_adopt::HOOK_TRIGGERS, &["pre-commit", "pre-push"]);
        for trigger in dx_adopt::HOOK_TRIGGERS {
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
                "powershell",
                "ruby",
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
                Command::Update => {
                    let mut want: Vec<String> = dx_update::sets::SetId::ALL
                        .iter()
                        .map(|id| (*id).name().to_owned())
                        .collect();
                    want.sort();
                    assert_eq!(fixed, want);
                    assert!(labels);
                }
                Command::Bump => {
                    let mut want: Vec<String> = dx_bump::BumpSet::names()
                        .into_iter()
                        .map(String::from)
                        .collect();
                    want.sort();
                    assert_eq!(fixed, want, "bump must offer the bump sets");
                    assert!(!fixed.contains(&String::from("uv")));
                    assert!(fixed.contains(&String::from("github-actions")));
                    assert!(!labels);
                }
                Command::Clean
                | Command::Init
                | Command::Upgrade
                | Command::Status
                | Command::Version
                | Command::Capabilities
                | Command::Verify
                | Command::Rerun
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
        assert_eq!(Command::value_variants().len(), 36);
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
            Command::Deploy,
        ] {
            assert_eq!(
                slot_candidates(command, &one, ""),
                (Vec::new(), false),
                "{command:?} must close after its slot"
            );
        }
        for command in [
            Command::Build,
            Command::Run,
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
    fn the_engine_offers_the_command_table_and_the_global_flags() {
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path();
        let commands = complete(&["dx", ""], root);
        for command in Command::value_variants() {
            assert!(
                commands.iter().any(|value| value == command.name()),
                "clap must offer dx {}: {commands:?}",
                command.name()
            );
        }
        let extra: Vec<&String> = commands
            .iter()
            .filter(|value| {
                !Command::value_variants()
                    .iter()
                    .any(|command| command.name() == value.as_str())
            })
            .collect();
        assert_eq!(
            extra,
            vec![&"--help".to_owned(), &"--version".to_owned()],
            "clap adds only its own flags to the command table"
        );
        assert!(complete(&["dx", "b"], root)
            .iter()
            .any(|value| value == "build"));
        assert!(!complete(&["dx", "b"], root)
            .iter()
            .any(|value| value == "test"));
        let flags = complete(&["dx", "build", "-"], root);
        for flag in [
            "--workspace",
            "--dry-run",
            "--quiet",
            "-v",
            "--color",
            "--log-level",
            "--output",
            "--debug",
            "--release",
            "--here",
            "-h",
        ] {
            assert!(
                flags.iter().any(|value| value == flag),
                "clap must offer {flag} for dx build: {flags:?}"
            );
        }
        for flag in [
            "--report",
            "--fail-on",
            "--min-coverage",
            "--strict-evidence",
            "--check",
            "--bazel",
            "--pin",
            "--rollback",
            "--configured",
            "--from",
            "--to",
            "--serve",
            "--port",
            "--host",
            "--open",
            "--offline",
        ] {
            assert!(
                !flags.iter().any(|value| value == flag),
                "clap must not offer {flag} for dx build: {flags:?}"
            );
        }
    }

    #[test]
    fn the_engine_offers_slots_labels_and_tasks() {
        let scratch = workspace_with_package();
        let root = scratch.path();
        let watch = complete(&["dx", "watch", ""], root);
        for task in dx_adopt::WATCHABLE_COMMANDS {
            assert!(
                watch.iter().any(|value| value == task),
                "watch must offer {task}: {watch:?}"
            );
        }
        let build = complete(&["dx", "build", ""], root);
        assert!(
            build.iter().any(|value| value == "//..."),
            "build must offer //...: {build:?}"
        );
        assert!(
            build.iter().any(|value| value == "//pkg/..."),
            "build must offer //pkg/...: {build:?}"
        );
        let filtered = complete(&["dx", "build", "//pkg"], root);
        assert!(
            filtered.iter().any(|value| value == "//pkg/..."),
            "build must filter //pkg: {filtered:?}"
        );
        assert!(
            !filtered.iter().any(|value| value.starts_with("//other")),
            "build must not offer //other: {filtered:?}"
        );
        let shifted = complete(&["dx", "build", "--workspace", "/tmp", ""], root);
        assert!(
            shifted.iter().any(|value| value == "//..."),
            "flags must not shift slots: {shifted:?}"
        );
        let triggers = complete(&["dx", "hooks", "run", ""], root);
        assert!(
            triggers.iter().any(|value| value == "pre-commit"),
            "hooks run must offer triggers: {triggers:?}"
        );
        let closed = complete(&["dx", "hooks", "install", ""], root);
        for trigger in dx_adopt::HOOK_TRIGGERS {
            assert!(
                !closed.iter().any(|value| value == trigger),
                "hooks install must close the slot: {closed:?}"
            );
        }
        let external = complete(&["dx", "build", "@repo//..."], root);
        assert!(
            !external.iter().any(|value| value.starts_with("//")),
            "external prefixes stay empty: {external:?}"
        );
        assert!(
            complete(&["dx", "build", ""], Path::new("/nonexistent-zzz"))
                .iter()
                .any(|value| value == "//..."),
            "a missing workspace still offers //..."
        );
    }

    #[test]
    fn every_shell_script_calls_back_into_dx() {
        for shell in COMPLETION_SHELLS {
            let script = render_completion(shell).expect("render");
            assert!(
                registers_callback(&script, shell),
                "the {shell} script must ask dx for candidates: {script}"
            );
            assert!(
                script.contains(COMPLETION_BIN),
                "the {shell} script must name the binary: {script}"
            );
        }
        assert!(render_completion("tcsh").is_err());
    }

    #[test]
    fn the_words_a_shell_passes_name_the_slot_being_completed() {
        let scratch = workspace_with_package();
        let root = scratch.path();
        let triggers = complete_from_a_shell(&["dx", "hooks", "run", ""], root);
        assert!(
            triggers.iter().any(|value| value == "pre-commit"),
            "hooks run must offer triggers: {triggers:?}"
        );
        assert!(
            !triggers.iter().any(|value| value == "install"),
            "hooks run must not offer verbs again: {triggers:?}"
        );
        assert!(
            complete_from_a_shell(&["dx", "hooks", "install", ""], root)
                .iter()
                .all(|value| value.starts_with("--")),
            "hooks install must close the slot"
        );
        let tasks = complete_from_a_shell(&["dx", "watch", ""], root);
        assert!(
            tasks.iter().any(|value| value == "build"),
            "watch must offer tasks in its first slot: {tasks:?}"
        );
        assert!(
            !tasks.iter().any(|value| value == "//..."),
            "watch must not offer labels in its first slot: {tasks:?}"
        );
        let labels = complete_from_a_shell(&["dx", "watch", "build", ""], root);
        assert!(
            labels.iter().any(|value| value == "//pkg/..."),
            "watch must offer labels after its task: {labels:?}"
        );
        let shifted = complete_from_a_shell(&["dx", "build", "--workspace", "/tmp", ""], root);
        assert!(
            shifted.iter().any(|value| value == "//..."),
            "flags must not shift slots: {shifted:?}"
        );
        let filtered = complete_from_a_shell(&["dx", "build", "//pkg", ""], root);
        assert!(
            filtered.iter().any(|value| value == "//pkg/..."),
            "a shell word must filter the labels: {filtered:?}"
        );
    }

    #[test]
    fn prior_scopes_reads_both_word_lists() {
        let words =
            |words: &[&str]| -> Vec<OsString> { words.iter().map(OsString::from).collect() };
        let empty: Vec<String> = Vec::new();
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "hooks", "run", ""])),
            ["run"]
        );
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "watch", "build", "//a:one", ""])),
            ["build", "//a:one"]
        );
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "build", "//pkg", ""])),
            ["//pkg"]
        );
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "why", "a.rs", ""])),
            ["a.rs"]
        );
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "lint", "--check", ""])),
            empty
        );
        assert_eq!(
            prior_scopes(&words(&["--", "dx", "build", "--workspace", "/tmp", ""])),
            empty
        );
        assert_eq!(prior_scopes(&words(&["lint", "--check", ""])), empty);
        assert_eq!(prior_scopes(&words(&["hooks", "run", ""])), ["run"]);
        assert_eq!(prior_scopes(&words(&["build", "--", "--jobs=4"])), empty);
        assert_eq!(prior_scopes(&[]), empty);
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
        assert!(label_candidates_from_workspace(root, "//pkg:target", 100).is_empty());
    }

    fn module_workspace() -> tempfile::TempDir {
        let scratch = tempfile::tempdir().expect("scratch");
        std::fs::write(scratch.path().join("MODULE.bazel"), "").expect("marker");
        scratch
    }

    fn workspace_with_packages(names: &[&str]) -> tempfile::TempDir {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        for name in names {
            std::fs::create_dir_all(root.join(name)).expect("mkdir");
            std::fs::write(root.join(format!("{name}/BUILD.bazel")), "").expect("marker");
        }
        scratch
    }

    fn words_of(argv: &[&str]) -> Vec<OsString> {
        argv.iter().map(OsString::from).collect()
    }

    #[test]
    fn workspace_override_reads_both_spellings() {
        let flag = workspace_override_from_words(&words_of(&["build", "--workspace=/tmp/ws", ""]));
        assert_eq!(flag, Some(PathBuf::from("/tmp/ws")));
        let flag =
            workspace_override_from_words(&words_of(&["build", "--workspace", "/tmp/ws", ""]));
        assert_eq!(flag, Some(PathBuf::from("/tmp/ws")));
        assert!(workspace_override_from_words(&words_of(&["build", ""])).is_none());
        assert!(
            workspace_override_from_words(&words_of(&["build", "--workspace", "--quiet", ""]))
                .is_none()
        );
    }

    #[test]
    fn workspace_override_ignores_shell_separator_and_dx() {
        let flag = workspace_override_from_words(&words_of(&[
            "--",
            "dx",
            "build",
            "--workspace",
            "/tmp/ws",
            "",
        ]));
        assert_eq!(flag, Some(PathBuf::from("/tmp/ws")));
        let flag = workspace_override_from_words(&words_of(&[
            "--",
            "dx",
            "build",
            "--workspace=/tmp/ws",
            "",
        ]));
        assert_eq!(flag, Some(PathBuf::from("/tmp/ws")));
        let flag = workspace_override_from_words(&words_of(&["--", "dx", "build", ""]));
        assert!(flag.is_none());
    }

    #[test]
    fn workspace_override_selects_other_workspace_packages() {
        let first = workspace_with_packages(&["alpha"]);
        let second = workspace_with_packages(&["beta"]);
        let second_root = second.path().to_string_lossy().into_owned();
        let got = complete(
            &["dx", "build", "--workspace", second_root.as_str(), ""],
            first.path(),
        );
        assert!(
            got.iter().any(|value| value == "//beta/..."),
            "override must list beta: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value == "//alpha/..."),
            "override must not list alpha: {got:?}"
        );
        let again = complete(&["dx", "build", ""], first.path());
        assert!(
            again.iter().any(|value| value == "//alpha/..."),
            "bare must list alpha: {again:?}"
        );
    }

    #[test]
    fn workspace_override_from_shell_selects_other_workspace() {
        let first = workspace_with_packages(&["alpha"]);
        let second = workspace_with_packages(&["beta"]);
        let second_root = second.path().to_string_lossy().into_owned();
        let got = complete_from_a_shell(
            &["dx", "build", "--workspace", second_root.as_str(), ""],
            first.path(),
        );
        assert!(
            got.iter().any(|value| value == "//beta/..."),
            "shell override must list beta: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value == "//alpha/..."),
            "shell override must not list alpha: {got:?}"
        );
    }

    #[test]
    fn nested_workspaces_are_excluded() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        std::fs::create_dir_all(root.join("pkg")).expect("mkdir");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("marker");
        std::fs::create_dir_all(root.join("nested/inner")).expect("mkdir");
        std::fs::write(root.join("nested/MODULE.bazel"), "").expect("marker");
        std::fs::write(root.join("nested/BUILD.bazel"), "").expect("marker");
        std::fs::write(root.join("nested/inner/BUILD.bazel"), "").expect("marker");
        let got = label_candidates_from_workspace(root, "", 100);
        assert!(
            got.iter().any(|value| value == "//pkg/..."),
            "outer package stays: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value.starts_with("//nested")),
            "nested workspace leaves: {got:?}"
        );
        let completed = complete(&["dx", "build", ""], root);
        assert!(!completed.iter().any(|value| value.starts_with("//nested")));
    }

    #[test]
    fn bazelignore_excludes_listed_dirs() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        for dir in ["kept", "ignored", "ignored/nested"] {
            std::fs::create_dir_all(root.join(dir)).expect("mkdir");
            std::fs::write(root.join(format!("{dir}/BUILD.bazel")), "").expect("marker");
        }
        std::fs::write(root.join(".bazelignore"), "ignored\n# kept stays\n").expect("ignore");
        let got = label_candidates_from_workspace(root, "", 100);
        assert!(
            got.iter().any(|value| value == "//kept/..."),
            "kept stays: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value.starts_with("//ignored")),
            "ignored leaves: {got:?}"
        );
        let ignores = read_bazelignore(root);
        assert_eq!(ignores, vec!["ignored".to_owned()]);
        assert!(is_ignored("ignored", &ignores));
        assert!(is_ignored("ignored/nested", &ignores));
        assert!(!is_ignored("kept", &ignores));
    }

    #[test]
    fn spaced_workspace_paths_complete() {
        let scratch = tempfile::tempdir().expect("scratch");
        let spaced = scratch.path().join("with space").join("ws");
        std::fs::create_dir_all(spaced.join("pkg")).expect("mkdir");
        std::fs::write(spaced.join("MODULE.bazel"), "").expect("marker");
        std::fs::write(spaced.join("BUILD.bazel"), "").expect("marker");
        std::fs::write(spaced.join("pkg/BUILD.bazel"), "").expect("marker");
        let other = workspace_with_packages(&["elsewhere"]);
        let spaced_text = spaced.to_string_lossy().into_owned();
        let got = complete(
            &["dx", "build", "--workspace", spaced_text.as_str(), ""],
            other.path(),
        );
        assert!(
            got.iter().any(|value| value == "//pkg/..."),
            "spaced workspace lists pkg: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value == "//elsewhere/..."),
            "spaced override hides start: {got:?}"
        );
    }

    #[test]
    fn new_packages_appear_without_cache() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        std::fs::create_dir_all(root.join("pkg/a")).expect("mkdir");
        std::fs::write(root.join("pkg/a/BUILD.bazel"), "").expect("marker");
        let first = label_candidates_from_workspace(root, "", 100);
        assert!(first.iter().any(|value| value == "//pkg/a/..."));
        assert!(!first.iter().any(|value| value == "//pkg/b/..."));
        std::fs::create_dir_all(root.join("pkg/b")).expect("mkdir");
        std::fs::write(root.join("pkg/b/BUILD.bazel"), "").expect("marker");
        let second = label_candidates_from_workspace(root, "", 100);
        assert!(second.iter().any(|value| value == "//pkg/a/..."));
        assert!(
            second.iter().any(|value| value == "//pkg/b/..."),
            "new package appears: {second:?}"
        );
    }

    #[test]
    fn large_trees_stay_bounded() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        for index in 0..250 {
            let dir = format!("pkg/{index:03}");
            std::fs::create_dir_all(root.join(&dir)).expect("mkdir");
            std::fs::write(root.join(format!("{dir}/BUILD.bazel")), "").expect("marker");
        }
        let got = label_candidates_from_workspace(root, "", 100);
        assert_eq!(got.len(), 100, "candidates truncate: {got:?}");
        let many = label_candidates_from_workspace(root, "//pkg/", 200);
        assert!(many.len() <= 200, "package dirs bound: {many:?}");
    }

    #[test]
    fn package_of_current_scopes_one_package() {
        assert_eq!(package_of_current("//pkg:target"), Some("//pkg".to_owned()));
        assert_eq!(
            package_of_current("//pkg/sub:foo"),
            Some("//pkg/sub".to_owned())
        );
        assert_eq!(package_of_current("//:main"), Some("//".to_owned()));
        assert!(package_of_current("//...").is_none());
        assert!(package_of_current("//pkg/...").is_none());
        assert!(package_of_current("//pkg").is_none());
        assert!(package_of_current("@repo//pkg:target").is_none());
        assert!(package_of_current("src/main.rs").is_none());
        assert!(package_of_current("").is_none());
    }

    #[test]
    fn query_expression_never_covers_the_workspace() {
        assert_eq!(query_expression("//pkg"), "kind(rule, //pkg:*)");
        assert_eq!(query_expression("//pkg/sub"), "kind(rule, //pkg/sub:*)");
        assert_eq!(query_expression("//"), "kind(rule, //:*)");
        for expression in [
            query_expression("//pkg"),
            query_expression("//pkg/sub"),
            query_expression("//"),
        ] {
            assert!(
                !expression.contains("//..."),
                "bounded query must not span //...: {expression}"
            );
            assert!(expression.starts_with("kind(rule, "));
        }
    }

    #[test]
    fn target_query_runs_once_per_package_and_filters() {
        use std::cell::RefCell;
        let seen: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let query = |_: &Path, package: &str| {
            seen.borrow_mut().push(package.to_owned());
            vec![
                format!("{package}:one"),
                format!("{package}:two"),
                format!("{package}:other"),
            ]
        };
        let scratch = module_workspace();
        let got = target_candidates_with_query(scratch.path(), "//pkg:tw", 100, &query);
        assert_eq!(got, vec!["//pkg:two".to_owned()]);
        assert_eq!(seen.borrow().as_slice(), ["//pkg".to_owned()]);
        let none = target_candidates_with_query(scratch.path(), "//pkg/...", 100, &query);
        assert!(none.is_empty());
        assert_eq!(seen.borrow().len(), 1, "patterns run no query");
        let empty = target_candidates_with_query(scratch.path(), "@repo//pkg:one", 100, &query);
        assert!(empty.is_empty());
        assert_eq!(seen.borrow().len(), 1, "external runs no query");
    }

    #[test]
    fn target_failures_degrade_to_patterns() {
        let failing = |_: &Path, _: &str| Vec::new();
        let scratch = workspace_with_packages(&["pkg"]);
        let got = target_candidates_with_query(scratch.path(), "//pkg:missing", 100, failing);
        assert!(got.is_empty());
        let patterns = label_candidates_from_workspace(scratch.path(), "", 100);
        assert!(
            patterns.iter().any(|value| value == "//pkg/..."),
            "patterns still offer: {patterns:?}"
        );
    }

    #[test]
    fn check_apply_and_wrapped_args_stay_consistent_with_workspace() {
        let first = workspace_with_packages(&["alpha"]);
        let second = workspace_with_packages(&["beta"]);
        let second_root = second.path().to_string_lossy().into_owned();
        let got = complete(
            &[
                "dx",
                "lint",
                "--workspace",
                second_root.as_str(),
                "--check",
                "",
            ],
            first.path(),
        );
        assert!(
            got.iter().any(|value| value == "//beta/..."),
            "flags keep workspace: {got:?}"
        );
        let wrapped = complete(&["dx", "build", "--", "--jobs=4", ""], first.path());
        assert!(
            wrapped.iter().any(|value| value == "//alpha/..."),
            "wrapped args keep labels: {wrapped:?}"
        );
    }
}
