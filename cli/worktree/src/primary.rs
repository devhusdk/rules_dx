//! The main-checkout rule: once `dx.primary` is set, Bazel work happens in slots.

use std::path::{Path, PathBuf};

use super::WorktreeError;
use super::git;

/// The git config key that holds the rule's mode.
pub const KEY: &str = "dx.primary";

/// What `dx.primary` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Everyone is refused in the main checkout.
    Protected,
    /// Only agent sessions are refused in the main checkout.
    Open,
}

impl Mode {
    fn of(value: &str) -> Option<Mode> {
        match value {
            "protected" => Some(Mode::Protected),
            "open" => Some(Mode::Open),
            _ => None,
        }
    }

    /// Whether this mode refuses a session with this environment.
    #[must_use]
    pub fn refuses(self, env_get: &dyn Fn(&str) -> Option<String>) -> bool {
        match self {
            Mode::Protected => true,
            Mode::Open => dx_process::is_agent(env_get),
        }
    }
}

/// What the rule reads for a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reading {
    /// Outside git, or `dx.primary` is unset: the rule is off.
    Off,
    /// The mode in force, with a note when `dx.primary` named no mode and counts as `protected`.
    On(Mode, Option<String>),
    /// Git could not answer, so the rule refuses with this reason.
    Unreadable(String),
}

/// The nearest `.git` entry in `dir` or a parent; without one there is no repository to read.
fn dot_git(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .map(|parent| parent.join(".git"))
        .find(|entry| entry.symlink_metadata().is_ok())
}

/// The reason given when git cannot tell whether the rule applies.
fn unreadable(detail: &str) -> String {
    format!(
        "cannot check {KEY}: {}. dx refuses Bazel work until git can read it",
        detail.trim()
    )
}

/// The raw `dx.primary` value of the repository `dir` is in; none when unset.
fn value(dir: &Path) -> Result<Option<String>, String> {
    let Some(output) = git::own_config(dir, &["config", "--get", KEY])? else {
        return without_git(dir).map(|()| None);
    };
    match output.code {
        Some(0) => Ok(Some(output.stdout_text().trim().to_owned())),
        Some(1) if output.stdout.is_empty() => Ok(None),
        _ => Err(format!(
            "git config --get {KEY} failed: {}",
            output.stderr_text().trim()
        )),
    }
}

/// Without git, refuses when the nearest `.git` directory has worktrees, an unreadable config, or `dx.primary`.
fn without_git(dir: &Path) -> Result<(), String> {
    let Some(dot_git) = dot_git(dir).filter(|entry| entry.is_dir()) else {
        return Ok(());
    };
    if dot_git.join("worktrees").symlink_metadata().is_ok() {
        return Err(format!(
            "git is not on PATH, and {} has worktrees",
            dot_git.display()
        ));
    }
    let config = dot_git.join("config");
    let text = match std::fs::read(&config) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "git is not on PATH, and {} cannot be read: {error}",
                config.display()
            ));
        }
    };
    if sets_primary(&String::from_utf8_lossy(&text)) {
        return Err(format!(
            "git is not on PATH, and {} sets {KEY}",
            config.display()
        ));
    }
    Ok(())
}

/// Whether git config `text` has a `primary` entry in a `[dx]` section; names compare without case, as git does.
fn sets_primary(text: &str) -> bool {
    let mut in_dx = false;
    for line in text.lines() {
        let mut rest = line.trim_start();
        if let Some(header) = rest.strip_prefix('[') {
            let Some((name, after)) = header.split_once(']') else {
                in_dx = false;
                continue;
            };
            in_dx = name.eq_ignore_ascii_case("dx");
            rest = after.trim_start();
        }
        let key = rest
            .split(|c: char| c == '=' || c.is_whitespace())
            .next()
            .unwrap_or_default();
        if in_dx && key.eq_ignore_ascii_case("primary") {
            return true;
        }
    }
    false
}

fn read(dir: &Path) -> Reading {
    if dot_git(dir).is_none() {
        return Reading::Off;
    }
    match value(dir) {
        Ok(None) => Reading::Off,
        Ok(Some(value)) => match Mode::of(&value) {
            Some(mode) => Reading::On(mode, None),
            None => Reading::On(
                Mode::Protected,
                Some(format!(
                    "{KEY} is {value:?}, which is neither protected nor open, so dx treats it as protected: "
                )),
            ),
        },
        Err(detail) => Reading::Unreadable(unreadable(&detail)),
    }
}

/// The top of the main checkout `dir` is in; none in a linked worktree. An error names why git could not tell.
fn main_checkout(dir: &Path) -> Result<Option<PathBuf>, String> {
    let output = git::own_config(
        dir,
        &[
            "rev-parse",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
        ],
    )?
    .ok_or_else(|| "git is not on PATH".to_owned())?;
    if output.code != Some(0) {
        return Err(format!(
            "git rev-parse failed: {}",
            output.stderr_text().trim()
        ));
    }
    let text = output.stdout_text();
    let mut lines = text.lines().map(str::trim);
    let (Some(top), Some(git_dir), Some(common)) = (lines.next(), lines.next(), lines.next())
    else {
        return Err(format!("git rev-parse printed {text:?}"));
    };
    // Older git prints these relative to `dir`; compare the resolved directories.
    let resolve = |path: &str| {
        std::fs::canonicalize(dir.join(path))
            .map_err(|error| format!("cannot resolve {path}: {error}"))
    };
    Ok((resolve(git_dir)? == resolve(common)?).then(|| PathBuf::from(top)))
}

/// The refusal a reading gives in `dir`; none when Bazel work may run there.
fn refusal_of(
    reading: Reading,
    dir: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let note = match reading {
        Reading::Off => return None,
        Reading::Unreadable(reason) => return Some(reason),
        Reading::On(mode, _) if !mode.refuses(env_get) => return None,
        Reading::On(_, note) => note.unwrap_or_default(),
    };
    match main_checkout(dir) {
        Ok(None) => None,
        Ok(Some(main)) => Some(format!(
            "{note}no Bazel in the main checkout {}. Work in a slot: tools/dx worktree add <branch>",
            main.display()
        )),
        Err(detail) => Some(unreadable(&detail)),
    }
}

/// Why Bazel work in `dir` is refused, without the `refused:` prefix; none when it may run.
#[must_use]
pub fn refusal(dir: &Path, env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    refusal_of(read(dir), dir, env_get)
}

/// The refusal text for `bazel clean` in a slot.
pub const SLOT_CLEAN: &str = "bazel clean in a slot drops its warm outputs and does not fix a stale cache. Rerun with --noremote_accept_cached, or release this slot and add a fresh one.";

/// Bazel startup options that may take their value as the next word.
const STARTUP_VALUES: &[&str] = &[
    "bazelrc",
    "command_port",
    "connect_timeout_secs",
    "digest_function",
    "experimental_cgroup_parent",
    "failure_detail_out",
    "host_jvm_args",
    "host_jvm_profile",
    "install_base",
    "install_md5",
    "invocation_policy",
    "io_nice_level",
    "local_startup_timeout_secs",
    "macos_qos_class",
    "max_idle_secs",
    "output_base",
    "output_user_root",
    "server_javabase",
    "server_jvm_out",
    "unix_digest_hash_attribute_name",
];

/// The Bazel command in `args`: the first word that is neither a startup option nor its value.
fn bazel_command(args: &[String]) -> Option<&str> {
    let mut words = args.iter().map(String::as_str);
    while let Some(word) = words.next() {
        if let Some(option) = word.strip_prefix("--") {
            if STARTUP_VALUES.contains(&option) {
                words.next();
            }
        } else if !word.starts_with('-') {
            return Some(word);
        }
    }
    None
}

/// Why one Bazel call in `dir` is refused: any call in a refused main checkout, or `clean` in a slot.
#[must_use]
pub fn bazel_refusal(
    dir: &Path,
    args: &[String],
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let reading = read(dir);
    if reading == Reading::Off {
        return None;
    }
    if bazel_command(args) == Some("clean") && super::lease_for_workspace(dir).is_some() {
        return Some(SLOT_CLEAN.to_owned());
    }
    refusal_of(reading, dir, env_get)
}

/// Sets `dx.primary` to `protected` when it is unset; returns the notice to print when it did.
pub(crate) fn protect(main: &Path) -> Result<Option<String>, WorktreeError> {
    if value(main)
        .map_err(|detail| WorktreeError::git(&["config", "--get", KEY], &detail))?
        .is_some()
    {
        return Ok(None);
    }
    git::run(main, &["config", KEY, "protected"])?;
    Ok(Some(format!(
        "the main checkout {} is now protected: dx refuses Bazel work there; run git config {KEY} open to refuse only agents",
        main.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn without_git_only_a_set_primary_slots_or_an_unreadable_config_refuse() {
        let scratch = dx_test_scratch::scratch("primary-without-git").unwrap();
        let config = |name: &str, text: &str| {
            let root = scratch.path().join(name);
            std::fs::create_dir_all(root.join(".git")).unwrap();
            std::fs::write(root.join(".git/config"), text).unwrap();
            root
        };
        let empty = scratch.path().join("empty");
        std::fs::create_dir_all(empty.join(".git/objects")).unwrap();
        let sub = empty.join("pkg");
        std::fs::create_dir_all(&sub).unwrap();
        let linked = scratch.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: /elsewhere\n").unwrap();
        for root in [
            empty.clone(),
            sub,
            linked,
            config("core", "[core]\n\tbare = false\n"),
            config(
                "elsewhere",
                "[dx \"x\"]\n\tprimary = open\n[remote \"primary\"]\n\turl = u\n[dxx]\n\tprimary = open\n",
            ),
        ] {
            assert_eq!(without_git(&root), Ok(()), "{root:?}");
        }
        for (root, reason) in [
            (
                config("set", "[core]\n[DX]\n\tPrimary = open\n"),
                "sets dx.primary",
            ),
            (config("inline", "[dx] primary\n"), "sets dx.primary"),
            (
                config("bare-key", "[dx]\nprimary=protected"),
                "sets dx.primary",
            ),
        ] {
            let refusal = without_git(&root).unwrap_err();
            assert!(refusal.contains(reason), "{root:?}: {refusal}");
        }
        let slots = config("slots", "");
        std::fs::create_dir_all(slots.join(".git/worktrees/1")).unwrap();
        assert!(without_git(&slots).unwrap_err().contains("has worktrees"));
        let unreadable = scratch.path().join("unreadable");
        std::fs::create_dir_all(unreadable.join(".git/config")).unwrap();
        assert!(
            without_git(&unreadable)
                .unwrap_err()
                .contains("cannot be read")
        );
    }

    #[test]
    fn the_bazel_command_skips_startup_options_and_their_values() {
        for (line, want) in [
            ("clean", Some("clean")),
            ("--output_base /x clean", Some("clean")),
            ("--output_base=/x clean --async", Some("clean")),
            ("--host_jvm_args -Xmx1g --batch clean", Some("clean")),
            ("--bazelrc a.rc --nohome_rc build //...", Some("build")),
            ("--batch", None),
            ("-h", None),
        ] {
            assert_eq!(bazel_command(&words(line)), want, "{line}");
        }
    }

    #[test]
    fn rule_reads_strip_config_the_environment_injects() {
        let command = git::own_config_command(Path::new("."), &["config", "--get", KEY]);
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        for name in ["GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS"] {
            assert!(removed.iter().any(|got| got == name), "{name}: {removed:?}");
        }
    }
}
