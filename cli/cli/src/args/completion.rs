use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use dx_process::lifecycle::{
    CapturePolicy, ChildOutcome, CleanupBudget, Descendants, EnvPolicy, Exit, SpawnSpec,
};

use clap_complete::engine::{ArgValueCompleter, CompletionCandidate, ValueCompleter};

use super::command::{Command, FirstSlot, COMMANDS};
use super::grammar::consumes_next;
use super::ArgsError;

pub const COMPLETION_SHELLS: &[&str] = &["bash", "zsh", "fish", "powershell"];

/// The variable a shell sets to ask dx for candidates.
pub const COMPLETE_VAR: &str = "COMPLETE";

/// The binary name the rendered scripts call back into.
const COMPLETION_BIN: &str = "dx";

const MAX_LABEL_CANDIDATES: usize = 100;

const MAX_PACKAGE_DIRS: usize = 200;

const MAX_VISITED_DIRS: usize = 2000;

const QUERY_TIMEOUT: Duration = Duration::from_millis(1200);

const QUERY_CLEANUP: CleanupBudget = CleanupBudget {
    graceful: Duration::from_millis(100),
    forced: Duration::from_millis(100),
    drain: Duration::from_millis(100),
};

const QUERY_STARTUP_OPTS: &[&str] = &["--noblock_for_lock"];

const QUERY_MAX_BYTES: usize = 256 * 1024;

/// Serve the shell request in COMPLETE, and report whether there was one.
pub fn try_complete() -> Result<bool, String> {
    clap_complete::CompleteEnv::with_factory(super::grammar::cli_command)
        .try_complete(
            std::env::args_os(),
            dx_process::current_directory().ok().as_deref(),
        )
        .map_err(|error| error.to_string())
}

/// Give every command the completer for its own scope slot.
pub fn with_scope_completers(
    command: clap::Command,
    words: impl IntoIterator<Item = OsString>,
    start: &Path,
) -> clap::Command {
    let request = Request {
        words: words.into_iter().collect(),
        start: start.to_path_buf(),
        env_workspace: std::env::var(super::DX_WORKSPACE_ENV).ok(),
        is_ci: dx_process::is_ci(),
    };
    with_request(command, request)
}

fn with_request(mut command: clap::Command, request: Request) -> clap::Command {
    let prior = prior_scopes(&request.words);
    let request = Arc::new(request);
    let selection: Arc<OnceLock<Selection>> = Arc::new(OnceLock::new());
    for entry in COMMANDS.iter() {
        let completer = ArgValueCompleter::new(ScopeCompleter {
            command: entry.command,
            prior: prior.clone(),
            request: Arc::clone(&request),
            selection: Arc::clone(&selection),
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
#[cfg(test)]
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
            let next = words.get(index).map(|next| next.to_string_lossy());
            if consumes_next(&word, next.as_deref()) {
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

/// The typed words, the start directory, and the environment one request reads.
struct Request {
    words: Vec<OsString>,
    start: PathBuf,
    env_workspace: Option<String>,
    is_ci: bool,
}

/// The workspace and Bazel startup tokens a request selects.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Selection {
    workspace: PathBuf,
    startup: Vec<String>,
}

impl Request {
    /// Select the way a run does, falling back to the start directory.
    fn select(&self) -> Selection {
        let (flag, tokens) =
            super::tokenizer::selected_identity(typed_words(&self.words)).unwrap_or_default();
        let flag = flag
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty());
        let raw = super::parser::startup_workspace(
            &self.start,
            flag,
            self.env_workspace.clone(),
            self.is_ci,
        );
        let workspace = dx_process::discover_real(&self.start, raw.as_deref().map(Path::new))
            .unwrap_or_else(|_| dx_process::canonicalize_or_keep(&self.start));
        dx_process::configure_real(&workspace);
        let startup = tokens
            .iter()
            .filter_map(|token| dx_process::validate_startup_option(token).ok())
            .collect();
        Selection { workspace, startup }
    }
}

/// The words after the program name, without the word in hand.
fn typed_words(words: &[OsString]) -> &[OsString] {
    let mut typed: &[OsString] = words;
    if typed.first().is_some_and(|first| first == "--") {
        typed = typed.get(2..).unwrap_or_default();
    } else if typed.first().is_some_and(|first| first == "dx") {
        typed = &typed[1..];
    }
    typed.split_last().map_or(typed, |(_, rest)| rest)
}

/// The candidates one command offers for its scope slot.
#[derive(Clone)]
struct ScopeCompleter {
    command: Command,
    prior: Vec<String>,
    request: Arc<Request>,
    selection: Arc<OnceLock<Selection>>,
}

impl ValueCompleter for ScopeCompleter {
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        self.complete_at(0, current)
    }

    fn complete_at(&self, _index: usize, current: &OsStr) -> Vec<CompletionCandidate> {
        let current = current.to_string_lossy();
        let (mut names, allows_labels) = slot_candidates(self.command, &self.prior, &current);
        if allows_labels {
            let selection = self.selection.get_or_init(|| self.request.select());
            names.extend(label_hint(&current));
            names.extend(label_candidates_from_workspace(
                &selection.workspace,
                &current,
                MAX_LABEL_CANDIDATES,
            ));
            names.extend(target_candidates(
                &selection.workspace,
                &selection.startup,
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

/// Whether the package sits in a nested workspace or an ignored directory.
fn in_excluded_dir(workspace: &Path, package: &str) -> bool {
    let rel = package.strip_prefix("//").unwrap_or(package);
    if rel.is_empty() {
        return false;
    }
    if is_ignored(rel, &read_bazelignore(workspace)) {
        return true;
    }
    let mut prefix = PathBuf::new();
    rel.split('/').any(|part| {
        prefix.push(part);
        is_workspace_root(&workspace.join(&prefix))
    })
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

/// The bounded query for one package under the selected startup identity.
fn query_spec(workspace: &Path, startup: &[String], package: &str) -> SpawnSpec {
    let mut argv: Vec<OsString> = vec![OsString::from(dx_process::launcher_argv0())];
    argv.extend(
        dx_process::current()
            .startup_opts()
            .iter()
            .chain(QUERY_STARTUP_OPTS)
            .map(OsString::from),
    );
    argv.extend(startup.iter().map(OsString::from));
    argv.extend(["query", "--output=label"].into_iter().map(OsString::from));
    argv.push(OsString::from(query_expression(package)));
    SpawnSpec {
        argv,
        cwd: workspace.to_path_buf(),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        capture: CapturePolicy {
            max_bytes: QUERY_MAX_BYTES,
        },
        timeout: QUERY_TIMEOUT,
        descendants: Descendants::Detachable,
    }
}

/// The labels one bounded query returns, empty when it cannot.
fn run_label_query(spec: &SpawnSpec) -> Vec<String> {
    match dx_process::lifecycle::run_within(spec, QUERY_CLEANUP) {
        Ok(ChildOutcome::Finished {
            exit: Exit::Code(0),
            stdout,
            ..
        }) => parse_labels(&stdout),
        _ => Vec::new(),
    }
}

/// The sorted, unique label lines in query output, empty when it is not text.
fn parse_labels(stdout: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(stdout) else {
        return Vec::new();
    };
    let mut out: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("//") && !line.contains(char::is_whitespace))
        .map(ToString::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The target labels matching the current word through one bounded query.
fn target_candidates(
    workspace: &Path,
    startup: &[String],
    current: &str,
    limit: usize,
) -> Vec<String> {
    target_candidates_with_query(workspace, current, limit, |workspace, package| {
        run_label_query(&query_spec(workspace, startup, package))
    })
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
    if in_excluded_dir(workspace, &package) {
        return Vec::new();
    }
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

fn add_set_hints(current: &str) -> Vec<String> {
    prefixed(&dx_bump::AddRequest::names(), current)
}

fn slot_candidates(command: Command, prior: &[String], current: &str) -> (Vec<String>, bool) {
    let meta = command.meta();
    let fixed = if prior.is_empty() {
        match meta.first_slot {
            FirstSlot::None => Vec::new(),
            FirstSlot::UpdateSets => update_set_hints(current),
            FirstSlot::BumpSets => bump_set_hints(current),
            FirstSlot::AddSets => add_set_hints(current),
            FirstSlot::NewLanguages => prefixed(dx_adopt::SUPPORTED_NEW_LANGUAGES, current),
            FirstSlot::WatchTasks => prefixed(dx_adopt::WATCHABLE_COMMANDS, current),
            FirstSlot::CompletionShells => prefixed(COMPLETION_SHELLS, current),
        }
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
            if dx_path::bazel_package::is_package_marker(&entry) {
                has_build = true;
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
        let request = Request {
            words: passed,
            start: workspace.to_path_buf(),
            env_workspace: None,
            is_ci: false,
        };
        let mut command = with_request(super::super::grammar::Cli::command(), request);
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
    fn a_short_cluster_value_is_not_a_prior_scope() {
        let words: Vec<OsString> = ["lint", "-qo", "json", "pkg", "x"]
            .map(OsString::from)
            .to_vec();
        assert_eq!(prior_scopes(&words), vec!["pkg".to_owned()]);
    }

    #[test]
    fn fixed_tables_match_their_single_sources() {
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
                Command::Add => {
                    let mut want: Vec<String> = dx_bump::AddRequest::names()
                        .into_iter()
                        .map(String::from)
                        .collect();
                    want.sort();
                    assert_eq!(fixed, want, "add must offer the add sets");
                    assert!(!fixed.contains(&String::from("github-actions")));
                    assert!(!labels);
                }
                Command::Clean
                | Command::Init
                | Command::Upgrade
                | Command::Status
                | Command::Version
                | Command::Rollback
                | Command::Worktree => {
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
    }

    #[test]
    fn later_slots_follow_per_command_shapes() {
        let task = vec![String::from("test")];
        assert_eq!(
            slot_candidates(Command::Watch, &task, ""),
            (Vec::new(), true)
        );
        let one = vec![String::from("//a:one")];
        for command in [
            Command::Codegen,
            Command::Env,
            Command::Bump,
            Command::Add,
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
            Command::Security,
            Command::License,
            Command::Update,
        ] {
            assert!(
                slot_candidates(command, &one, "").1,
                "{command:?} must stay open for more scopes"
            );
        }
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
            "-q",
            "-v",
            "--color",
            "--build-slots",
            "-o",
            "--debug",
            "--release",
            "--report",
            "--here",
            "-h",
        ] {
            assert!(
                flags.iter().any(|value| value == flag),
                "clap must offer {flag} for dx build: {flags:?}"
            );
        }
        for flag in [
            "--fail-on",
            "--min-coverage",
            "--check",
            "--bazel",
            "--configured",
            "--from",
            "--to",
            "--serve",
            "--port",
            "--host",
            "--open",
            "--offline",
            "--frozen",
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
            prior_scopes(&words(&["--", "dx", "watch", "build", ""])),
            ["build"]
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
            prior_scopes(&words(&["--", "dx", "list-tests", "a.rs", ""])),
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
        assert_eq!(prior_scopes(&words(&["watch", "build", ""])), ["build"]);
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
        {
            std::os::unix::fs::symlink(root.join("pkg/a"), root.join("linked")).expect("symlink");
            std::fs::write(root.join("linked/BUILD.bazel"), "").expect("marker");
        }
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

    #[cfg(unix)]
    #[test]
    fn label_scan_counts_symlinked_build_files() {
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path();
        for dir in ["pkg/c", "pkg/d/out", "pkg/e", "pkg/f"] {
            std::fs::create_dir_all(root.join(dir)).expect("mkdir");
        }
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        std::fs::write(root.join("pkg/c/real.bzl"), "").expect("marker");
        std::os::unix::fs::symlink("real.bzl", root.join("pkg/c/BUILD")).expect("symlink");
        std::os::unix::fs::symlink("out", root.join("pkg/d/BUILD.bazel")).expect("symlink");
        std::os::unix::fs::symlink("missing", root.join("pkg/e/BUILD")).expect("symlink");
        std::os::unix::fs::symlink("../c/real.bzl", root.join("pkg/f/BUILD.bazel"))
            .expect("symlink");
        let mut got = label_candidates_from_workspace(root, "", 100);
        got.sort();
        assert_eq!(got, vec!["//pkg/c/...", "//pkg/f/..."]);
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

    fn select(words: &[&str], start: &Path, env_workspace: Option<&str>, is_ci: bool) -> Selection {
        Request {
            words: words_of(words),
            start: start.to_path_buf(),
            env_workspace: env_workspace.map(str::to_owned),
            is_ci,
        }
        .select()
    }

    fn real(path: &Path) -> PathBuf {
        dx_process::canonicalize_or_keep(path)
    }

    #[test]
    fn the_workspace_flag_selects_in_both_spellings() {
        let start = workspace_with_packages(&["alpha"]);
        let other = workspace_with_packages(&["beta"]);
        let other_text = other.path().to_string_lossy().into_owned();
        let joined = format!("--workspace={other_text}");
        for words in [
            vec!["build", joined.as_str(), ""],
            vec!["build", "--workspace", other_text.as_str(), ""],
            vec!["--", "dx", "build", "--workspace", other_text.as_str(), ""],
            vec!["--", "./bin/dx", "build", joined.as_str(), "//pkg:"],
        ] {
            assert_eq!(
                select(&words, start.path(), None, false).workspace,
                real(other.path()),
                "{words:?}"
            );
        }
        for words in [
            vec!["build", ""],
            vec!["--", "dx", "build", ""],
            vec!["build", "--workspace", "--quiet", ""],
            vec!["build", "--bogus", "--workspace", other_text.as_str(), ""],
        ] {
            assert_eq!(
                select(&words, start.path(), None, false).workspace,
                real(start.path()),
                "{words:?}"
            );
        }
    }

    #[test]
    fn the_environment_and_config_select_like_a_run() {
        let start = workspace_with_packages(&["alpha"]);
        let env = workspace_with_packages(&["beta"]);
        let file = workspace_with_packages(&["gamma"]);
        let env_text = env.path().to_string_lossy().into_owned();
        let file_text = file.path().to_string_lossy().into_owned();
        let bare = ["build", ""];
        assert_eq!(
            select(&bare, start.path(), Some(&env_text), false).workspace,
            real(env.path())
        );
        assert_eq!(
            select(&bare, start.path(), Some(&env_text), true).workspace,
            real(start.path()),
            "CI ignores DX_WORKSPACE"
        );
        assert_eq!(
            select(&bare, start.path(), Some(""), false).workspace,
            real(start.path())
        );
        std::fs::write(
            start.path().join("dx.toml"),
            format!("[dx]\nworkspace = '{file_text}'\n"),
        )
        .expect("config");
        assert_eq!(
            select(&bare, start.path(), None, false).workspace,
            real(file.path())
        );
        assert_eq!(
            select(&bare, start.path(), Some(&env_text), false).workspace,
            real(env.path()),
            "the environment beats the config"
        );
        let flagged = ["build", "--workspace", env_text.as_str(), ""];
        assert_eq!(
            select(&flagged, start.path(), None, false).workspace,
            real(env.path()),
            "the flag beats the config"
        );
        let got = complete_in(&["dx", "build", ""], start.path(), None);
        assert!(
            got.iter().any(|value| value == "//gamma/..."),
            "config workspace lists gamma: {got:?}"
        );
        assert!(
            !got.iter().any(|value| value == "//alpha/..."),
            "config workspace hides alpha: {got:?}"
        );
        let got = complete_in(&["dx", "build", ""], start.path(), Some(&env_text));
        assert!(
            got.iter().any(|value| value == "//beta/..."),
            "environment workspace lists beta: {got:?}"
        );
    }

    #[test]
    fn broken_defaults_complete_quietly_from_the_start_workspace() {
        let start = workspace_with_packages(&["alpha"]);
        std::fs::write(start.path().join("dx.toml"), "not toml = [").expect("config");
        assert_eq!(
            select(&["build", ""], start.path(), None, false).workspace,
            real(start.path())
        );
        let got = complete_in(&["dx", "build", ""], start.path(), None);
        assert!(
            got.iter().any(|value| value == "//alpha/..."),
            "broken defaults still list alpha: {got:?}"
        );
        let missing = start.path().join("missing");
        std::fs::write(
            start.path().join("dx.toml"),
            format!("[dx]\nworkspace = '{}'\n", missing.display()),
        )
        .expect("config");
        assert_eq!(
            select(&["build", ""], start.path(), None, false).workspace,
            real(start.path())
        );
    }

    fn complete_in(argv: &[&str], start: &Path, env_workspace: Option<&str>) -> Vec<String> {
        let words: Vec<OsString> = argv.iter().map(OsString::from).collect();
        let mut passed = vec![OsString::from("--")];
        passed.extend(words.iter().cloned());
        let request = Request {
            words: passed,
            start: start.to_path_buf(),
            env_workspace: env_workspace.map(str::to_owned),
            is_ci: false,
        };
        let mut command = with_request(super::super::grammar::Cli::command(), request);
        clap_complete::engine::complete(&mut command, words.clone(), words.len() - 1, Some(start))
            .expect("complete")
            .iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect()
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
    fn package_cap_stops_the_walk() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        for index in 0..250 {
            let dir = format!("pkg/{index:03}");
            std::fs::create_dir_all(root.join(&dir)).expect("mkdir");
            std::fs::write(root.join(format!("{dir}/BUILD.bazel")), "").expect("marker");
        }
        let expected = |count: usize| -> Vec<String> {
            (0..count).map(|i| format!("//pkg/{i:03}/...")).collect()
        };
        let got = label_candidates_from_workspace(root, "", 100);
        assert_eq!(got, expected(100));
        let many = label_candidates_from_workspace(root, "//pkg/", 250);
        assert_eq!(many, expected(MAX_PACKAGE_DIRS));
        assert!(!many.contains(&"//pkg/200/...".to_owned()), "{many:?}");
    }

    #[test]
    fn visited_cap_stops_the_walk() {
        let scratch = module_workspace();
        let root = scratch.path();
        std::fs::write(root.join("BUILD.bazel"), "").expect("marker");
        for index in 0..1997 {
            std::fs::create_dir_all(root.join(format!("a/{index:04}"))).expect("mkdir");
        }
        std::fs::create_dir_all(root.join("z")).expect("mkdir");
        std::fs::write(root.join("z/BUILD.bazel"), "").expect("marker");
        assert_eq!(
            label_candidates_from_workspace(root, "", 100),
            vec!["//z/...".to_owned()]
        );
        std::fs::create_dir_all(root.join("a/1997")).expect("mkdir");
        assert!(label_candidates_from_workspace(root, "", 100).is_empty());
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
        let got = target_candidates_with_query(scratch.path(), "//pkg:tw", 100, query);
        assert_eq!(got, vec!["//pkg:two".to_owned()]);
        assert_eq!(seen.borrow().as_slice(), ["//pkg".to_owned()]);
        let none = target_candidates_with_query(scratch.path(), "//pkg/...", 100, query);
        assert!(none.is_empty());
        assert_eq!(seen.borrow().len(), 1, "patterns run no query");
        let empty = target_candidates_with_query(scratch.path(), "@repo//pkg:one", 100, query);
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
    fn target_query_skips_nested_workspaces_and_ignored_dirs() {
        use std::cell::RefCell;
        let scratch = module_workspace();
        let root = scratch.path();
        for dir in ["pkg", "nested/inner", "deep/x", "ignored/sub"] {
            std::fs::create_dir_all(root.join(dir)).expect("mkdir");
        }
        for file in [
            "pkg/BUILD.bazel",
            "nested/MODULE.bazel",
            "nested/inner/BUILD.bazel",
            "deep/WORKSPACE.bazel",
            "deep/x/BUILD.bazel",
            "ignored/sub/BUILD.bazel",
        ] {
            std::fs::write(root.join(file), "").expect("marker");
        }
        std::fs::write(root.join(".bazelignore"), "ignored\n").expect("ignore");
        let seen: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let query = |_: &Path, package: &str| {
            seen.borrow_mut().push(package.to_owned());
            vec![format!("{package}:a")]
        };
        for current in [
            "//nested:a",
            "//nested/inner:a",
            "//deep/x:a",
            "//ignored/sub:a",
        ] {
            let got = target_candidates_with_query(root, current, 100, query);
            assert!(got.is_empty(), "{current} offers nothing: {got:?}");
        }
        assert!(seen.borrow().is_empty(), "excluded dirs run no query");
        let got = target_candidates_with_query(root, "//pkg:a", 100, query);
        assert_eq!(got, vec!["//pkg:a".to_owned()]);
        let got = target_candidates_with_query(root, "//:a", 100, query);
        assert_eq!(got, vec!["//:a".to_owned()]);
        assert_eq!(
            seen.borrow().as_slice(),
            ["//pkg".to_owned(), "//".to_owned()]
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

    #[test]
    fn startup_tokens_follow_the_grammar_in_order() {
        let start = workspace_with_packages(&["pkg"]);
        let got = select(
            &[
                "--",
                "dx",
                "build",
                "--bazel-startup-option=--output_base=/tmp/a b/\u{fc}",
                "--bazel-startup-option=--output_user_root=/tmp/u",
                "--bazel-startup-option=--output_base=/tmp/last",
                "//pkg:",
            ],
            start.path(),
            None,
            false,
        );
        assert_eq!(
            got.startup,
            vec![
                "--output_base=/tmp/a b/\u{fc}",
                "--output_user_root=/tmp/u",
                "--output_base=/tmp/last",
            ]
        );
        let separated = select(
            &[
                "build",
                "--bazel-startup-option",
                "--output_user_root=/tmp/u",
                "//pkg:",
            ],
            start.path(),
            None,
            false,
        );
        assert_eq!(
            separated.startup,
            vec!["--output_user_root=/tmp/u"],
            "a run takes a separated dashed token"
        );
    }

    #[test]
    fn startup_tokens_skip_invalid_the_word_in_hand_and_the_payload() {
        let start = workspace_with_packages(&["pkg"]);
        let got = select(
            &[
                "build",
                "--bazel-startup-option=--bogus=1",
                "--bazel-startup-option=--output_base",
                "--bazel-startup-option=--output_base=/tmp/last",
            ],
            start.path(),
            None,
            false,
        );
        assert!(got.startup.is_empty(), "{:?}", got.startup);
        let got = select(
            &[
                "build",
                "--",
                "--bazel-startup-option=--output_base=/tmp/payload",
                "",
            ],
            start.path(),
            None,
            false,
        );
        assert!(got.startup.is_empty(), "{:?}", got.startup);
    }

    #[test]
    fn the_query_puts_startup_tokens_before_the_verb() {
        let start = workspace_with_packages(&["pkg"]);
        let startup = vec![
            "--output_base=/tmp/a b/\u{fc}".to_owned(),
            "--output_user_root=/tmp/u".to_owned(),
        ];
        let spec = query_spec(start.path(), &startup, "//pkg");
        let argv: Vec<String> = spec
            .argv
            .iter()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            argv,
            vec![
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--noblock_for_lock",
                "--output_base=/tmp/a b/\u{fc}",
                "--output_user_root=/tmp/u",
                "query",
                "--output=label",
                "kind(rule, //pkg:*)",
            ]
        );
        assert_eq!(spec.cwd, start.path());
        assert_eq!(spec.timeout, QUERY_TIMEOUT);
        assert_eq!(spec.capture.max_bytes, QUERY_MAX_BYTES);
        assert_eq!(spec.descendants, Descendants::Detachable);
        assert!(
            QUERY_TIMEOUT + QUERY_CLEANUP.total() <= Duration::from_millis(1500),
            "the whole query fits the interactive budget"
        );
    }

    fn probe_spec(flags: &[&str], max_bytes: usize) -> SpawnSpec {
        let mut argv = vec![dx_testing::process_probe().into_os_string()];
        argv.extend(flags.iter().map(OsString::from));
        SpawnSpec {
            argv,
            cwd: std::env::temp_dir(),
            env: EnvPolicy::Inherited { extra: Vec::new() },
            capture: CapturePolicy { max_bytes },
            timeout: QUERY_TIMEOUT,
            descendants: Descendants::Detachable,
        }
    }

    const HANG_GUARD: Duration = Duration::from_secs(10);

    #[test]
    fn a_probe_listing_labels_yields_whole_label_records() {
        let spec = probe_spec(
            &["--stdout-text=//pkg:two\n//pkg:one\nnoise line\n//pkg:one\n"],
            QUERY_MAX_BYTES,
        );
        assert_eq!(run_label_query(&spec), vec!["//pkg:one", "//pkg:two"]);
        let failing = probe_spec(
            &["--stdout-text=//pkg:one\n", "--exit-code=7"],
            QUERY_MAX_BYTES,
        );
        assert!(run_label_query(&failing).is_empty());
    }

    #[test]
    fn a_probe_past_the_byte_bound_falls_back_without_waiting() {
        let spec = probe_spec(&["--stdout-bytes=1048576"], QUERY_MAX_BYTES);
        let started = std::time::Instant::now();
        let outcome = dx_process::lifecycle::run_within(&spec, QUERY_CLEANUP);
        assert!(
            matches!(outcome, Ok(ChildOutcome::OutputTooLarge { .. })),
            "output past pipe capacity must drain while the child runs: {outcome:?}"
        );
        assert!(run_label_query(&spec).is_empty());
        assert!(started.elapsed() < HANG_GUARD, "{:?}", started.elapsed());
    }

    #[test]
    fn a_blocked_probe_is_stopped() {
        let scratch = tempfile::tempdir().expect("scratch");
        let lock = scratch.path().join("blocked.alive");
        let spec = probe_spec(
            &[
                "--stdout-text=//pkg:late\n",
                "--sleep-ms=30000",
                &format!("--alive-lock={}", lock.display()),
            ],
            QUERY_MAX_BYTES,
        );
        let started = std::time::Instant::now();
        assert!(run_label_query(&spec).is_empty());
        assert!(started.elapsed() < HANG_GUARD, "{:?}", started.elapsed());
        dx_testing::assert_alive_lock_released(&lock);
    }

    #[test]
    fn a_descendant_holding_stdout_is_stopped() {
        let scratch = tempfile::tempdir().expect("scratch");
        let lock = scratch.path().join("descendant.alive");
        let spec = probe_spec(
            &[
                "--stdout-text=//pkg:one\n",
                "--detached-descendant",
                "--descendant-sleep-ms=30000",
                &format!("--descendant-alive-lock={}", lock.display()),
            ],
            QUERY_MAX_BYTES,
        );
        let started = std::time::Instant::now();
        assert!(run_label_query(&spec).is_empty());
        assert!(started.elapsed() < HANG_GUARD, "{:?}", started.elapsed());
        dx_testing::assert_alive_lock_released(&lock);
    }

    #[test]
    fn query_output_keeps_only_whole_label_lines() {
        let got = parse_labels(b"//a:two\n//a:one\n//a:one\nWARNING: noise here\n\n");
        assert_eq!(got, vec!["//a:one", "//a:two"]);
        assert!(parse_labels(&[0xff, 0xfe]).is_empty());
    }
}
