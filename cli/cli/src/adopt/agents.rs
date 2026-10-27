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
    let test = Command::Test.name();
    format!(
        "{BEGIN}\n\
         - Start: `{dx} {worktree} add <branch>`. Finish: `{dx} {worktree} release`.\n\
         - Fix first: `{dx} {fix} --apply --affected`.\n\
         - Then verify: `{dx} {test} --affected`.\n\
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
    let end = if text[end..].starts_with("\r\n") {
        end + 2
    } else if text[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}

fn orphaned(text: &str) -> bool {
    text.contains(BEGIN) && span(text).is_none()
}

pub(super) fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn block_for(text: &str) -> String {
    block().replace('\n', eol(text))
}

/// Whether the text holds the current block, an old one, or none.
pub(crate) fn state(text: &str) -> State {
    match span(text) {
        None => State::Missing,
        Some((start, end)) if text[start..end].replace("\r\n", "\n") == block() => State::Current,
        Some(_) => State::Stale,
    }
}

/// The text with the block added or replaced and every other byte kept.
pub(crate) fn render(text: &str) -> String {
    let block = block_for(text);
    match span(text) {
        Some((start, end)) => format!("{}{}{}", &text[..start], block, &text[end..]),
        None => {
            let eol = eol(text);
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(eol);
            }
            if !out.is_empty() {
                out.push_str(eol);
            }
            out.push_str(&block);
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

/// The AGENTS.md text a write would produce, or `None` when nothing would change.
pub(crate) fn planned(workspace: &Path, create: bool) -> io::Result<Option<String>> {
    let current = read(workspace)?;
    if current.is_none() && !create {
        return Ok(None);
    }
    let text = current.unwrap_or_default();
    if orphaned(&text) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{FILE} has {BEGIN} without {END}; add the end marker or remove the begin marker"
            ),
        ));
    }
    if state(&text) == State::Current {
        return Ok(None);
    }
    Ok(Some(render(&text)))
}

/// Writes the block, creating AGENTS.md when `create` is set. True when the file changed.
pub(crate) fn write(workspace: &Path, create: bool) -> io::Result<bool> {
    let Some(text) = planned(workspace, create)? else {
        return Ok(false);
    };
    dx_atomic_fs::write_atomic(&target(workspace, FILE)?, text.as_bytes())?;
    Ok(true)
}

pub(super) fn target(workspace: &Path, rel: &str) -> io::Result<std::path::PathBuf> {
    let path = workspace.join(rel);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => match std::fs::canonicalize(&path) {
            Ok(real) => Ok(real),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(workspace.join(std::fs::read_link(&path)?))
            }
            Err(error) => Err(error),
        },
        _ => Ok(path),
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
            "tools/dx fix --apply --affected",
            "tools/dx test --affected",
            "--output=json",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }

    #[test]
    fn the_block_commands_are_real_dx_commands() {
        for command in [Command::Worktree, Command::Fix, Command::Test] {
            assert!(Command::parse(command.name()).is_some());
        }
        for command in [Command::Fix, Command::Test] {
            assert!(
                crate::args::grammar::advertised_flags(command).contains(&"affected".to_owned())
            );
        }
    }

    #[test]
    fn the_block_fix_command_writes_and_test_command_runs_affected_tests() {
        let text = block();
        let line = |prefix: &str| -> Vec<String> {
            let line = text
                .lines()
                .find(|line| line.starts_with(prefix))
                .unwrap_or_else(|| panic!("{prefix}: {text}"));
            let command = line.split('`').nth(1).expect("command");
            command
                .split_whitespace()
                .skip(1)
                .map(str::to_owned)
                .collect()
        };
        let fix = crate::args::parse(&line("- Fix first:")).expect("fix parses");
        assert_eq!(fix.command, Command::Fix);
        assert!(fix.applies() && fix.affected);
        let test = crate::args::parse(&line("- Then verify:")).expect("test parses");
        assert_eq!(test.command, Command::Test);
        assert!(!test.applies() && test.affected);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_file_stays_a_symlink() {
        let dir = scratch("dx-agents-symlink-");
        std::fs::write(dir.path().join("real.md"), "# Mine\n").expect("seed");
        std::os::unix::fs::symlink("real.md", dir.path().join(FILE)).expect("link");
        assert!(write(dir.path(), false).expect("write"));
        let meta = std::fs::symlink_metadata(dir.path().join(FILE)).expect("meta");
        assert!(meta.file_type().is_symlink());
        let text = std::fs::read_to_string(dir.path().join("real.md")).expect("read");
        assert!(text.starts_with("# Mine\n"), "{text}");
        assert_eq!(state(&text), State::Current);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_created_through_the_link() {
        let dir = scratch("dx-agents-dangling-");
        std::os::unix::fs::symlink("real.md", dir.path().join(FILE)).expect("link");
        assert!(write(dir.path(), true).expect("write"));
        let meta = std::fs::symlink_metadata(dir.path().join(FILE)).expect("meta");
        assert!(meta.file_type().is_symlink());
        let text = std::fs::read_to_string(dir.path().join("real.md")).expect("read");
        assert_eq!(text, block());
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
        assert!(write(dir.path(), false).expect("write"));
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
    fn a_block_without_an_end_marker_is_refused_and_the_file_kept() {
        let dir = scratch("dx-agents-orphan-");
        let old = format!("# Mine\n{BEGIN}\nuser text\n");
        std::fs::write(dir.path().join(FILE), &old).expect("seed");
        let error = write(dir.path(), true).expect_err("orphan");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).expect("read"),
            old
        );
    }

    #[test]
    fn planned_reads_without_writing_and_fails_on_an_orphan_or_a_directory() {
        let dir = scratch("dx-agents-planned-");
        assert_eq!(planned(dir.path(), false).expect("absent"), None);
        assert_eq!(planned(dir.path(), true).expect("create"), Some(block()));
        assert!(!dir.path().join(FILE).exists());
        let old = format!("# Mine\n{BEGIN}\n");
        std::fs::write(dir.path().join(FILE), &old).expect("seed");
        assert!(planned(dir.path(), true).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).expect("read"),
            old
        );
        let nested = scratch("dx-agents-planned-dir-");
        std::fs::create_dir(nested.path().join(FILE)).expect("dir");
        assert!(planned(nested.path(), true).is_err());
    }

    #[test]
    fn a_crlf_file_with_the_current_block_is_current() {
        let text = format!("# Mine\r\n\r\n{}", block().replace('\n', "\r\n"));
        assert_eq!(state(&text), State::Current);
    }

    #[test]
    fn a_crlf_file_keeps_crlf_when_the_block_is_written() {
        let stale = format!("# Mine\r\n{BEGIN}\r\n- old\r\n{END}\r\nTail\r\n");
        assert_eq!(state(&stale), State::Stale);
        let out = render(&stale);
        assert_eq!(
            out,
            format!("# Mine\r\n{}Tail\r\n", block().replace('\n', "\r\n"))
        );
        assert_eq!(state(&out), State::Current);
        let appended = render("# Mine\r\n");
        assert!(!appended.replace("\r\n", "").contains('\n'), "{appended:?}");
    }
}
