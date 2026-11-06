use std::io;
use std::path::Path;

use dx_atomic_fs::{PointerKind, create_pointer};

/// Points `dir/name` at `target`, replacing any earlier link in one rename.
pub fn link(dir: &Path, name: &str, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let kind = if target.is_dir() {
        PointerKind::Directory
    } else {
        PointerKind::File
    };
    place(dir, name, target, kind)?;
    if cfg!(windows) && kind == PointerKind::File {
        place(dir, &format!("{name}.exe"), target, kind)?;
    }
    Ok(())
}

fn place(dir: &Path, name: &str, target: &Path, kind: PointerKind) -> io::Result<()> {
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let _ = remove(&temp);
    create_pointer(target, &temp, kind)?;
    let final_path = dir.join(name);
    if cfg!(windows) {
        let _ = remove(&final_path);
    }
    std::fs::rename(&temp, &final_path).inspect_err(|_| {
        let _ = remove(&temp);
    })
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(_) if path.is_dir() => std::fs::remove_dir(path),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[test]
    #[cfg(unix)]
    fn link_replaces_the_old_target() {
        let scratch = dx_test_scratch::scratch("dx-ide-link-");
        let first = scratch.path().join("first");
        let second = scratch.path().join("second");
        std::fs::create_dir_all(&first).expect("first");
        std::fs::create_dir_all(&second).expect("second");
        let dir = scratch.path().join("out");
        link(&dir, "lib", &first).expect("link");
        link(&dir, "lib", &second).expect("relink");
        assert_eq!(std::fs::read_link(dir.join("lib")).expect("read"), second);
        scratch.close().expect("cleanup");
    }
}

#[cfg(unix)]
pub(crate) fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
pub(crate) fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
