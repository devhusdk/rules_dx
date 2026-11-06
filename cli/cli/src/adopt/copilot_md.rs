//! The pointer to `AGENTS.md` in `.github/copilot-instructions.md`.

use std::io;
use std::path::Path;

use super::agents::{FILE as AGENTS, eol, target};

pub(crate) const FILE: &str = ".github/copilot-instructions.md";

/// The text init owns; Copilot has no import syntax, so it names the file in prose.
pub(crate) const BLOCK: &str = "Follow the rules in AGENTS.md. Run dx commands through tools/dx.\n";

fn read(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Whether the file exists and points at AGENTS.md.
pub(crate) fn imported(workspace: &Path) -> io::Result<bool> {
    Ok(read(&workspace.join(FILE))?.is_some_and(|text| text.contains(AGENTS)))
}

/// Adds the block when the file does not mention AGENTS.md, keeping every other byte. True when the file changed.
pub(crate) fn write(workspace: &Path) -> io::Result<bool> {
    if imported(workspace)? {
        return Ok(false);
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
    out.push_str(&BLOCK.replace('\n', eol));
    let path = target(workspace, FILE)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    dx_atomic_fs::write_atomic(&path, out.as_bytes())?;
    Ok(true)
}

/// Like [`write`], but leaves an absent file absent.
pub(crate) fn refresh(workspace: &Path) -> io::Result<bool> {
    if std::fs::symlink_metadata(workspace.join(FILE)).is_err() {
        return Ok(false);
    }
    write(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::scratch;

    #[test]
    fn an_absent_file_is_created_and_an_existing_one_gets_the_block_appended() {
        let dir = scratch("dx-copilot-md-");
        assert!(!refresh(dir.path()).expect("absent"));
        assert!(write(dir.path()).expect("create"));
        assert!(!write(dir.path()).expect("again"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).expect("read"),
            BLOCK
        );
        std::fs::write(dir.path().join(FILE), "# Mine\n").expect("seed");
        assert!(!imported(dir.path()).expect("state"));
        assert!(refresh(dir.path()).expect("append"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).expect("read"),
            format!("# Mine\n\n{BLOCK}")
        );
    }
}
