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

/// The raw `dx.primary` value of the repository `dir` is in; none when unset or outside git.
fn value(dir: &Path) -> Option<String> {
    git::line(dir, &["config", "--get", KEY]).ok()
}

/// The mode of the repository `dir` is in; none when `dx.primary` is unset or names no mode.
#[must_use]
pub fn mode(dir: &Path) -> Option<Mode> {
    value(dir).as_deref().and_then(Mode::of)
}

/// The top of the main checkout `dir` is in; none in a linked worktree, a bare repository or outside git.
#[must_use]
pub fn main_checkout(dir: &Path) -> Option<PathBuf> {
    let text = git::line(
        dir,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ],
    )
    .ok()?;
    let mut lines = text.lines().map(str::trim);
    let (git_dir, common, top) = (lines.next()?, lines.next()?, lines.next()?);
    (git_dir == common && !top.is_empty()).then(|| PathBuf::from(top))
}

/// Why Bazel work in `dir` is refused, without the `refused:` prefix; none when it may run or in CI.
#[must_use]
pub fn refusal(dir: &Path, ci: bool, env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if ci || !mode(dir)?.refuses(env_get) {
        return None;
    }
    main_refusal(dir)
}

fn main_refusal(dir: &Path) -> Option<String> {
    let main = main_checkout(dir)?;
    Some(format!(
        "no Bazel in the main checkout {}. Work in a slot: tools/dx worktree add <branch>",
        main.display()
    ))
}

/// The refusal text for `bazel clean` in a slot.
pub const SLOT_CLEAN: &str = "bazel clean in a slot drops its warm outputs and does not fix a stale cache. Rerun with --noremote_accept_cached, or release this slot and add a fresh one.";

/// The Bazel command in `args`: the first word that is not a startup option.
fn bazel_command(args: &[String]) -> Option<&str> {
    args.iter()
        .map(String::as_str)
        .find(|word| !word.starts_with('-'))
}

/// Why one Bazel call in `dir` is refused: any call in a refused main checkout, or `clean` in a slot.
#[must_use]
pub fn bazel_refusal(
    dir: &Path,
    args: &[String],
    ci: bool,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if ci {
        return None;
    }
    let mode = mode(dir)?;
    if bazel_command(args) == Some("clean") && super::lease_for_workspace(dir).is_some() {
        return Some(SLOT_CLEAN.to_owned());
    }
    if !mode.refuses(env_get) {
        return None;
    }
    main_refusal(dir)
}

/// Sets `dx.primary` to `protected` when it is unset; returns the notice to print when it did.
pub(crate) fn protect(main: &Path) -> Result<Option<String>, WorktreeError> {
    if value(main).is_some() {
        return Ok(None);
    }
    git::run(main, &["config", KEY, "protected"])?;
    Ok(Some(format!(
        "the main checkout {} is now protected: dx refuses Bazel work there; run git config {KEY} open to refuse only agents",
        main.display()
    )))
}
