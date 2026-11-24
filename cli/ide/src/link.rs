use std::ffi::OsStr;
use std::io;
use std::path::Path;

use dx_atomic_fs::{PointerKind, replace_pointer};

pub fn link(dir: &Path, name: impl AsRef<OsStr>, target: &Path) -> io::Result<()> {
    let name = name.as_ref();
    let kind = if target.is_dir() {
        PointerKind::Directory
    } else {
        PointerKind::File
    };
    replace_pointer(target, &dir.join(name), kind)?;
    if cfg!(windows) && kind == PointerKind::File {
        let mut exe = name.to_os_string();
        exe.push(".exe");
        replace_pointer(target, &dir.join(exe), kind)?;
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[test]
    #[cfg(unix)]
    fn link_replaces_the_old_target() {
        let scratch = dx_test_scratch::scratch("dx-ide-link-").unwrap();
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

#[cfg(not(unix))]
pub(crate) fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
