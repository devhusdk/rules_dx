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

/// Whether `dir` or a parent holds a `.git` entry; without one there is no repository to read.
fn in_git(dir: &Path) -> bool {
    dir.ancestors()
        .any(|parent| parent.join(".git").symlink_metadata().is_ok())
}

/// The reason given when git cannot tell whether the rule applies.
fn unreadable(detail: &str) -> String {
    format!(
        "cannot check {KEY}: {}. dx refuses Bazel work until git can read it",
        detail.trim()
    )
}

/// The raw `dx.primary` value of the repository `dir` is in; none when unset or when git is not installed.
fn value(dir: &Path) -> Result<Option<String>, String> {
    // Only git and `dx worktree add` set the key, so a machine without git has neither the key nor slots to protect.
    let Some(output) = git::own_config(dir, &["config", "--get", KEY])? else {
        return Ok(None);
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

fn read(dir: &Path) -> Reading {
    if !in_git(dir) {
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
