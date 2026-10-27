//! The `@AGENTS.md` import in `CLAUDE.md`.

use std::io;
use std::path::Path;

use super::agents::{eol, target, FILE as AGENTS};

pub(crate) const FILE: &str = "CLAUDE.md";
const NESTED: &str = ".claude/CLAUDE.md";
const IMPORT: &str = "@AGENTS.md";

/// How the workspace memory file relates to AGENTS.md.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// A CLAUDE.md imports AGENTS.md.
    Imported,
    /// CLAUDE.md is a symlink to AGENTS.md.
    Linked,
    /// A CLAUDE.md exists without the import.
    Missing,
    /// No CLAUDE.md exists.
    Absent,
}

/// Whether `text` has `token` outside fenced code blocks and backticks.
fn has_token(text: &str, tokens: &[&str]) -> bool {
    let mut fence: Option<char> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let marker = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };
        if let Some(marker) = marker {
            match fence {
                None => fence = Some(marker),
                Some(open) if open == marker => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let outside: String = line
            .split('`')
            .enumerate()
            .filter(|(index, _)| index % 2 == 0)
            .map(|(_, part)| part)
            .collect::<Vec<_>>()
            .join(" ");
        if outside
            .split_whitespace()
            .any(|word| tokens.contains(&word))
        {
            return true;
        }
    }
    false
}

fn read(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn links_to_agents(workspace: &Path) -> bool {
    let path = workspace.join(FILE);
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return false;
    };
    if !meta.file_type().is_symlink() {
        return false;
    }
    match (
        std::fs::canonicalize(&path),
        std::fs::canonicalize(workspace.join(AGENTS)),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => std::fs::read_link(&path)
            .is_ok_and(|link| link == Path::new(AGENTS) || link == Path::new("./AGENTS.md")),
    }
}

/// Whether the workspace CLAUDE.md files import AGENTS.md.
pub(crate) fn state(workspace: &Path) -> io::Result<State> {
    if links_to_agents(workspace) {
        return Ok(State::Linked);
    }
    let top = read(&workspace.join(FILE))?;
    let nested = read(&workspace.join(NESTED))?;
    if top
        .as_deref()
        .is_some_and(|text| has_token(text, &[IMPORT, "@./AGENTS.md"]))
        || nested
            .as_deref()
            .is_some_and(|text| has_token(text, &["@../AGENTS.md"]))
    {
        return Ok(State::Imported);
    }
    Ok(if top.is_some() || nested.is_some() {
        State::Missing
    } else {
        State::Absent
    })
}

/// Adds the import when absent, keeping every other byte. True when the file changed.
pub(crate) fn write(workspace: &Path) -> io::Result<bool> {
    match state(workspace)? {
        State::Imported | State::Linked => return Ok(false),
        State::Missing | State::Absent => {}
    }
    let text = read(&workspace.join(FILE))?.unwrap_or_default();
    let eol = eol(&text);
    let mut out = text.clone();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    if !out.is_empty() {
        out.push_str(eol);
    }
    out.push_str(IMPORT);
    out.push_str(eol);
    dx_atomic_fs::write_atomic(&target(workspace, FILE)?, out.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    use dx_test_scratch::scratch;

    fn put(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    }

    fn get(dir: &Path, rel: &str) -> String {
        std::fs::read_to_string(dir.join(rel)).expect("read")
    }

    #[test]
    fn an_absent_file_is_created() {
        let dir = scratch("dx-claude-md-absent-");
        assert_eq!(state(dir.path()).expect("state"), State::Absent);
        assert!(write(dir.path()).expect("write"));
        assert_eq!(get(dir.path(), FILE), "@AGENTS.md\n");
        assert!(!write(dir.path()).expect("again"));
    }

    #[test]
    fn a_directory_in_place_of_the_file_is_an_error() {
        let dir = scratch("dx-claude-md-dir-");
        std::fs::create_dir(dir.path().join(FILE)).expect("dir");
        assert!(state(dir.path()).is_err());
    }

    #[test]
    fn an_existing_file_gets_the_line_appended() {
        let dir = scratch("dx-claude-md-append-");
        put(dir.path(), FILE, "# Mine\n");
        assert!(write(dir.path()).expect("write"));
        assert_eq!(get(dir.path(), FILE), "# Mine\n\n@AGENTS.md\n");
        put(dir.path(), FILE, "# Mine\r\ntext");
        assert!(write(dir.path()).expect("write"));
        assert_eq!(
            get(dir.path(), FILE),
            "# Mine\r\ntext\r\n\r\n@AGENTS.md\r\n"
        );
    }

    #[test]
    fn an_existing_import_is_left_alone() {
        let dir = scratch("dx-claude-md-present-");
        put(dir.path(), FILE, "Read @AGENTS.md first\n");
        assert_eq!(state(dir.path()).expect("state"), State::Imported);
        assert!(!write(dir.path()).expect("write"));
        assert_eq!(get(dir.path(), FILE), "Read @AGENTS.md first\n");
    }

    #[test]
    fn imports_in_fences_and_backticks_do_not_count() {
        let dir = scratch("dx-claude-md-fence-");
        put(
            dir.path(),
            FILE,
            "```\n@AGENTS.md\n```\nUse `@AGENTS.md` here\n",
        );
        assert_eq!(state(dir.path()).expect("state"), State::Missing);
        assert!(write(dir.path()).expect("write"));
        assert_eq!(state(dir.path()).expect("state"), State::Imported);
    }

    #[test]
    fn the_nested_file_counts() {
        let dir = scratch("dx-claude-md-nested-");
        put(dir.path(), NESTED, "@../AGENTS.md\n");
        assert_eq!(state(dir.path()).expect("state"), State::Imported);
        assert!(!write(dir.path()).expect("write"));
        assert!(!dir.path().join(FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_agents_is_never_written() {
        let dir = scratch("dx-claude-md-link-");
        put(dir.path(), AGENTS, "# Agents\n");
        std::os::unix::fs::symlink(AGENTS, dir.path().join(FILE)).expect("link");
        assert_eq!(state(dir.path()).expect("state"), State::Linked);
        assert!(!write(dir.path()).expect("write"));
        assert_eq!(get(dir.path(), AGENTS), "# Agents\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_to_agents_is_never_written() {
        let dir = scratch("dx-claude-md-dangling-");
        std::os::unix::fs::symlink(AGENTS, dir.path().join(FILE)).expect("link");
        assert!(!write(dir.path()).expect("write"));
        assert!(!dir.path().join(AGENTS).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_another_file_is_written_through() {
        let dir = scratch("dx-claude-md-other-");
        put(dir.path(), "docs/memory.md", "# Memory\n");
        std::os::unix::fs::symlink("docs/memory.md", dir.path().join(FILE)).expect("link");
        assert!(write(dir.path()).expect("write"));
        assert_eq!(
            get(dir.path(), "docs/memory.md"),
            "# Memory\n\n@AGENTS.md\n"
        );
        assert!(std::fs::symlink_metadata(dir.path().join(FILE))
            .expect("meta")
            .file_type()
            .is_symlink());
    }
}
