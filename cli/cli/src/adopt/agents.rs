use std::io;
use std::path::Path;

use crate::args::Command;

pub(crate) const FILE: &str = "AGENTS.md";
const BEGIN: &str = "<!-- dx:begin -->";
const END: &str = "<!-- dx:end -->";

/// The marked block that tells agents which dx commands to run.
pub(crate) fn block() -> String {
    let dx = "tools/dx";
    let worktree = Command::Worktree.name();
    let fix = Command::Fix.name();
    let check = Command::Check.name();
    format!(
        "{BEGIN}\n\
         - Start: `{dx} {worktree} add <branch>`. Finish: `{dx} {worktree} release`.\n\
         - Fix first: `{dx} {fix} --affected`.\n\
         - Then verify: `{dx} {check} --affected`.\n\
         - Read the `--output=json` result.\n\
         {END}\n"
    )
}

/// How an AGENTS.md text relates to the current block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum State {
    Missing,
    Stale,
    Current,
}

fn span(text: &str) -> Option<(usize, usize)> {
    let start = text.find(BEGIN)?;
    let end = start + text[start..].find(END)? + END.len();
    let end = if text[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}

/// Whether the text holds the current block, an old one, or none.
pub(crate) fn state(text: &str) -> State {
    match span(text) {
        None => State::Missing,
        Some((start, end)) if text[start..end] == block() => State::Current,
        Some(_) => State::Stale,
    }
}

/// The text with the block added or replaced and every other byte kept.
pub(crate) fn render(text: &str) -> String {
    match span(text) {
        Some((start, end)) => format!("{}{}{}", &text[..start], block(), &text[end..]),
        None => {
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&block());
            out
        }
    }
}

fn read(workspace: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(workspace.join(FILE)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// The block state of the workspace AGENTS.md.
pub(crate) fn workspace_state(workspace: &Path) -> io::Result<State> {
    Ok(read(workspace)?.map_or(State::Missing, |text| state(&text)))
}

/// Writes the block, creating AGENTS.md when `create` is set. True when the file changed.
pub(crate) fn write(workspace: &Path, create: bool) -> io::Result<bool> {
    let current = read(workspace)?;
    if current.is_none() && !create {
        return Ok(false);
    }
    let text = current.unwrap_or_default();
    if state(&text) == State::Current {
        return Ok(false);
    }
    std::fs::write(workspace.join(FILE), render(&text))?;
    Ok(true)
}

/// Refreshes an existing block and leaves a file without one alone.
pub(crate) fn refresh(workspace: &Path) -> io::Result<bool> {
    match read(workspace)? {
        Some(text) if state(&text) != State::Missing => write(workspace, false),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dx_test_scratch::scratch;

    #[test]
    fn the_block_is_short_and_names_the_agent_commands() {
        let text = block();
        assert!(text.lines().count() <= 10, "{text}");
        for want in [
            "tools/dx worktree add <branch>",
            "tools/dx worktree release",
            "tools/dx fix --affected",
            "tools/dx check --affected",
            "--output=json",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }

    #[test]
    fn the_block_commands_are_real_dx_commands() {
        for command in [Command::Worktree, Command::Fix, Command::Check] {
            assert!(Command::parse(command.name()).is_some());
        }
        assert!(Command::Fix.supports_affected() && Command::Check.supports_affected());
    }

    #[test]
    fn a_missing_file_is_created_with_the_block() {
        let dir = scratch("dx-agents-create-");
        assert!(write(dir.path(), true).expect("write"));
        let text = std::fs::read_to_string(dir.path().join(FILE)).expect("read");
        assert_eq!(text, block());
        assert_eq!(state(&text), State::Current);
        assert!(!write(dir.path(), true).expect("again"));
    }

    #[test]
    fn a_stale_block_is_replaced_and_other_text_is_kept() {
        let dir = scratch("dx-agents-stale-");
        let old = format!("# Mine\n\nkeep\n\n{BEGIN}\n- old\n{END}\n\n## After\nkeep too\n");
        std::fs::write(dir.path().join(FILE), &old).expect("seed");
        assert_eq!(workspace_state(dir.path()).expect("state"), State::Stale);
        assert!(refresh(dir.path()).expect("refresh"));
        let text = std::fs::read_to_string(dir.path().join(FILE)).expect("read");
        assert_eq!(
            text,
            format!("# Mine\n\nkeep\n\n{}\n## After\nkeep too\n", block())
        );
        assert_eq!(workspace_state(dir.path()).expect("state"), State::Current);
    }

    #[test]
    fn a_file_without_a_block_gets_one_appended() {
        let dir = scratch("dx-agents-append-");
        std::fs::write(dir.path().join(FILE), "# Mine\nno newline").expect("seed");
        assert_eq!(workspace_state(dir.path()).expect("state"), State::Missing);
        assert!(write(dir.path(), true).expect("write"));
        let text = std::fs::read_to_string(dir.path().join(FILE)).expect("read");
        assert_eq!(text, format!("# Mine\nno newline\n\n{}", block()));
    }

    #[test]
    fn refresh_leaves_a_file_without_a_block_alone() {
        let dir = scratch("dx-agents-refresh-none-");
        assert!(!refresh(dir.path()).expect("absent"));
        assert!(!dir.path().join(FILE).exists());
        std::fs::write(dir.path().join(FILE), "# Mine\n").expect("seed");
        assert!(!refresh(dir.path()).expect("no block"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).expect("read"),
            "# Mine\n"
        );
    }
}
