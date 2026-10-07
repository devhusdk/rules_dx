//! Managed filesystem operations with explicit link and replacement semantics.

use std::io;
use std::path::Path;

/// What a managed path is, classified without following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Missing,
    File,
    Directory,
    Link,
}

/// How a materialized file may be represented at its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkPolicy {
    /// The destination must become a link; a refused link is the caller's error.
    RequireLink,
    /// Prefer a link and copy the bytes when the host refuses to create one.
    LinkOrCopy,
}

/// The representation one materialization actually used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    Linked,
    Copied,
}

/// Whether a managed pointer names a file or a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    File,
    Directory,
}

/// Classifies one managed path without following a link.
pub fn classify(path: &Path) -> io::Result<EntryKind> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Ok(EntryKind::Link),
        Ok(meta) if meta.is_dir() => Ok(EntryKind::Directory),
        Ok(_) => Ok(EntryKind::File),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(EntryKind::Missing),
        Err(err) => Err(err),
    }
}

/// Materializes an immutable file at its destination under an explicit policy.
pub fn materialize_file(
    source: &Path,
    destination: &Path,
    policy: LinkPolicy,
) -> io::Result<Mechanism> {
    materialize_file_with(source, destination, policy, &link_file)
}

/// Materializes a file with the link attempt supplied by the caller.
pub fn materialize_file_with(
    source: &Path,
    destination: &Path,
    policy: LinkPolicy,
    link: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<Mechanism> {
    match classify(source)? {
        EntryKind::File | EntryKind::Link => {}
        EntryKind::Missing => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("source {} does not exist", source.display()),
            ));
        }
        EntryKind::Directory => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source {} is a directory", source.display()),
            ));
        }
    }
    match classify(destination)? {
        EntryKind::Missing | EntryKind::File => {}
        EntryKind::Link => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "destination {} is a link; refusing to write through it",
                    destination.display()
                ),
            ));
        }
        EntryKind::Directory => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "destination {} is a directory; refusing to replace a tree",
                    destination.display()
                ),
            ));
        }
    }
    if matches!(policy, LinkPolicy::LinkOrCopy) {
        match link(source, destination) {
            Ok(()) => return Ok(Mechanism::Linked),
            Err(_) => return copy_file(source, destination),
        }
    }
    link(source, destination)?;
    Ok(Mechanism::Linked)
}

/// Removes one managed entry without following it into its target.
pub fn remove_managed(path: &Path) -> io::Result<()> {
    match classify(path)? {
        EntryKind::Missing => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} does not exist", path.display()),
        )),
        EntryKind::Directory => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} is a directory; remove the tree deliberately instead",
                path.display()
            ),
        )),
        EntryKind::File => std::fs::remove_file(path),
        EntryKind::Link => unlink_entry(path),
    }
}

/// Replaces the managed pointer at `pointer` with one naming `target`.
pub fn replace_pointer(pointer: &Path, target: &Path, kind: PointerKind) -> io::Result<()> {
    match classify(pointer)? {
        EntryKind::Missing => {}
        EntryKind::Link => unlink_entry(pointer)?,
        EntryKind::File | EntryKind::Directory => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is not a managed pointer; refusing to adopt foreign state",
                    pointer.display()
                ),
            ));
        }
    }
    create_pointer(target, pointer, kind)
}

fn copy_file(source: &Path, destination: &Path) -> io::Result<Mechanism> {
    std::fs::copy(source, destination).map(|_| Mechanism::Copied)
}

fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

pub(crate) fn create_pointer(target: &Path, pointer: &Path, kind: PointerKind) -> io::Result<()> {
    #[cfg(windows)]
    {
        match kind {
            PointerKind::File => std::os::windows::fs::symlink_file(target, pointer),
            PointerKind::Directory => std::os::windows::fs::symlink_dir(target, pointer),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = kind;
        std::os::unix::fs::symlink(target, pointer)
    }
}

fn unlink_entry(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const DIRECTORY_ATTRIBUTE: u32 = 0x10;
        if std::fs::symlink_metadata(path)?.file_attributes() & DIRECTORY_ATTRIBUTE != 0 {
            return std::fs::remove_dir(path);
        }
        std::fs::remove_file(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::scratch;
    use std::path::PathBuf;

    fn refuses_link(_target: &Path, _pointer: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "symlink creation needs privileges",
        ))
    }

    #[cfg(windows)]
    fn directory_pointer(target: &Path, pointer: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(target, pointer)
    }

    #[cfg(not(windows))]
    fn directory_pointer(target: &Path, pointer: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, pointer)
    }

    #[cfg(windows)]
    fn file_pointer(target: &Path, pointer: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(target, pointer)
    }

    #[cfg(not(windows))]
    fn file_pointer(target: &Path, pointer: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, pointer)
    }

    fn skip_without_link_privileges(result: io::Result<()>) -> bool {
        if result.is_ok() {
            return false;
        }
        eprintln!("host refuses links; the copy path covers this case instead");
        true
    }

    #[test]
    fn classifies_regular_entries_without_following_links() {
        let scratch = scratch("dx-fs-classify-");
        let file = scratch.path().join("a.txt");
        std::fs::write(&file, b"a\n").expect("write file");
        let dir = scratch.path().join("d");
        std::fs::create_dir(&dir).expect("create dir");
        assert_eq!(classify(&file).expect("file"), EntryKind::File);
        assert_eq!(classify(&dir).expect("dir"), EntryKind::Directory);
        assert_eq!(
            classify(&scratch.path().join("absent")).expect("missing"),
            EntryKind::Missing
        );
        assert_eq!(
            classify(&scratch.path()).expect("root"),
            EntryKind::Directory
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn classifies_links_and_dangling_links() {
        let scratch = scratch("dx-fs-classify-link-");
        let outside = scratch.path().join("outside");
        std::fs::create_dir(&outside).expect("outside dir");
        let file = scratch.path().join("f.txt");
        std::fs::write(&file, b"f\n").expect("outside file");
        let file_link = scratch.path().join("file-link");
        let dir_link = scratch.path().join("dir-link");
        let dangling = scratch.path().join("dangling");
        std::os::unix::fs::symlink(&file, &file_link).expect("file link");
        std::os::unix::fs::symlink(&outside, &dir_link).expect("dir link");
        std::os::unix::fs::symlink(scratch.path().join("gone"), &dangling).expect("dangling");
        assert_eq!(classify(&file_link).expect("file link"), EntryKind::Link);
        assert_eq!(
            classify(&dir_link).expect("dir link"),
            EntryKind::Link,
            "a directory link is a link, not a directory"
        );
        assert_eq!(classify(&dangling).expect("dangling"), EntryKind::Link);
        assert!(!scratch.path().join("gone").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn materializes_bytes_and_reports_the_mechanism() {
        let scratch = scratch("dx-fs-materialize-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("nested").join("copy.toml");
        std::fs::create_dir(destination.parent().expect("parent")).expect("parent");
        assert_eq!(
            materialize_file_with(&source, &destination, LinkPolicy::LinkOrCopy, &refuses_link)
                .expect("copy fallback"),
            Mechanism::Copied
        );
        assert_eq!(
            std::fs::read(&destination).expect("copied bytes"),
            b"config = true\n"
        );
        assert_eq!(classify(&destination).expect("copy"), EntryKind::File);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn prefers_a_link_and_keeps_its_identity() {
        let scratch = scratch("dx-fs-materialize-link-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("linked.toml");
        assert_eq!(
            materialize_file(&source, &destination, LinkPolicy::LinkOrCopy).expect("link"),
            Mechanism::Linked
        );
        assert_eq!(
            std::fs::read_link(&destination).expect("link identity"),
            source
        );
        assert_eq!(classify(&destination).expect("link"), EntryKind::Link);
        std::fs::write(&source, b"config = false\n").expect("rewrite source");
        assert_eq!(
            std::fs::read(&destination).expect("linked bytes"),
            b"config = false\n",
            "the link keeps naming the source"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn copied_executables_stay_executable() {
        let scratch = scratch("dx-fs-materialize-exec-");
        let source = scratch.path().join("run.sh");
        std::fs::write(&source, b"#!/bin/sh\necho dx\n").expect("source");
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
                .expect("chmod source");
        }
        let destination = scratch.path().join("copied.sh");
        assert_eq!(
            materialize_file_with(&source, &destination, LinkPolicy::LinkOrCopy, &refuses_link)
                .expect("copy fallback"),
            Mechanism::Copied
        );
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&destination)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755,
                "the fallback keeps the executable mode"
            );
        }
        let output = std::process::Command::new(&destination)
            .output()
            .expect("run the copied script");
        assert!(output.status.success(), "copied script runs");
        assert_eq!(output.stdout, b"dx\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn rejects_a_refused_link_when_identity_is_required() {
        let scratch = scratch("dx-fs-require-link-");
        let source = scratch.path().join("pointer.bin");
        std::fs::write(&source, b"tool\n").expect("source");
        let destination = scratch.path().join("current");
        let err = materialize_file_with(
            &source,
            &destination,
            LinkPolicy::RequireLink,
            &refuses_link,
        )
        .expect_err("no copy may stand in for a link");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(err.to_string(), "symlink creation needs privileges");
        assert_eq!(
            classify(&destination).expect("destination"),
            EntryKind::Missing,
            "a required link never leaves a copy behind"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replaces_an_existing_destination_with_a_copy() {
        let scratch = scratch("dx-fs-replace-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("existing.toml");
        std::fs::write(&destination, b"stale\n").expect("existing");
        assert_eq!(
            materialize_file(&source, &destination, LinkPolicy::LinkOrCopy).expect("replace"),
            Mechanism::Copied,
            "an occupied name cannot become a link, so the fallback copies"
        );
        assert_eq!(
            std::fs::read(&destination).expect("replaced bytes"),
            b"config = true\n"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn refuses_a_directory_destination_and_keeps_the_tree() {
        let scratch = scratch("dx-fs-dir-dest-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("kept");
        std::fs::create_dir(&destination).expect("destination dir");
        std::fs::write(destination.join("inner.toml"), b"inner\n").expect("inner");
        let err = materialize_file(&source, &destination, LinkPolicy::LinkOrCopy)
            .expect_err("a tree is never replaced");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read(destination.join("inner.toml")).expect("inner kept"),
            b"inner\n"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn refuses_a_link_destination_and_keeps_the_target() {
        let scratch = scratch("dx-fs-link-dest-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let outside = scratch.path().join("outside.toml");
        std::fs::write(&outside, b"secret\n").expect("outside");
        let destination = scratch.path().join("current.toml");
        std::os::unix::fs::symlink(&outside, &destination).expect("plant link");
        let err = materialize_file(&source, &destination, LinkPolicy::LinkOrCopy)
            .expect_err("a managed link is never written through");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read(&outside).expect("outside"),
            b"secret\n",
            "the link target keeps its bytes"
        );
        assert_eq!(
            std::fs::read_link(&destination).expect("link survives"),
            outside
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn rejects_a_missing_source_and_a_directory_source() {
        let scratch = scratch("dx-fs-source-");
        let destination = scratch.path().join("out.toml");
        let err = materialize_file(
            &scratch.path().join("absent"),
            &destination,
            LinkPolicy::LinkOrCopy,
        )
        .expect_err("missing source");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(
            classify(&destination).expect("destination"),
            EntryKind::Missing
        );
        let err = materialize_file(&scratch.path(), &destination, LinkPolicy::LinkOrCopy)
            .expect_err("directory source");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn reports_a_missing_destination_parent() {
        let scratch = scratch("dx-fs-parent-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("absent").join("out.toml");
        for policy in [LinkPolicy::LinkOrCopy, LinkPolicy::RequireLink] {
            let err = materialize_file(&source, &destination, policy).expect_err("no parent");
            assert_eq!(err.kind(), io::ErrorKind::NotFound, "{policy:?}");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn preserves_the_error_of_a_read_only_destination() {
        let scratch = scratch("dx-fs-readonly-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("locked.toml");
        std::fs::write(&destination, b"stale\n").expect("existing");
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o444))
                .expect("chmod 444");
        }
        let err = materialize_file(&source, &destination, LinkPolicy::LinkOrCopy)
            .expect_err("a read-only destination is not forced");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            std::fs::read(&destination).expect("bytes kept"),
            b"stale\n",
            "the refused replacement leaves the file alone"
        );
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o644))
                .expect("chmod back");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn preserves_the_error_of_a_read_only_destination_parent() {
        let scratch = scratch("dx-fs-readonly-parent-");
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let parent = scratch.path().join("locked");
        std::fs::create_dir(&parent).expect("parent");
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555))
                .expect("chmod 555");
        }
        let err = materialize_file(&source, &parent.join("out.toml"), LinkPolicy::LinkOrCopy)
            .expect_err("a read-only parent is not forced");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))
                .expect("chmod back");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn removes_a_file_and_reports_missing_paths_and_trees() {
        let scratch = scratch("dx-fs-remove-");
        let file = scratch.path().join("plain.txt");
        std::fs::write(&file, b"x\n").expect("write");
        remove_managed(&file).expect("remove file");
        assert_eq!(classify(&file).expect("gone"), EntryKind::Missing);
        let err = remove_managed(&file).expect_err("missing is not a removal");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        let dir = scratch.path().join("tree");
        std::fs::create_dir(&dir).expect("tree");
        std::fs::write(dir.join("inner"), b"x\n").expect("inner");
        let err = remove_managed(&dir).expect_err("a tree is never removed");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(dir.join("inner").exists(), "the tree survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn removes_links_without_following_them() {
        let scratch = scratch("dx-fs-remove-link-");
        let outside_file = scratch.path().join("outside.txt");
        std::fs::write(&outside_file, b"secret\n").expect("outside file");
        let outside_dir = scratch.path().join("outside-dir");
        std::fs::create_dir(&outside_dir).expect("outside dir");
        std::fs::write(outside_dir.join("inner.txt"), b"inner\n").expect("inner");
        let file_link = scratch.path().join("file-link");
        let dir_link = scratch.path().join("dir-link");
        let dangling = scratch.path().join("dangling");
        std::os::unix::fs::symlink(&outside_file, &file_link).expect("file link");
        std::os::unix::fs::symlink(&outside_dir, &dir_link).expect("dir link");
        std::os::unix::fs::symlink(scratch.path().join("gone"), &dangling).expect("dangling");
        for link in [&file_link, &dir_link, &dangling] {
            remove_managed(link).expect("remove link");
            assert_eq!(classify(link).expect("link gone"), EntryKind::Missing);
        }
        assert_eq!(
            std::fs::read(&outside_file).expect("file target"),
            b"secret\n"
        );
        assert_eq!(
            std::fs::read(outside_dir.join("inner.txt")).expect("dir target"),
            b"inner\n"
        );
        assert!(outside_dir.is_dir(), "the directory target survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replaces_a_pointer_and_refuses_foreign_state() {
        let scratch = scratch("dx-fs-pointer-");
        let first = scratch.path().join("first");
        let second = scratch.path().join("second");
        std::fs::create_dir(&first).expect("first dir");
        std::fs::create_dir(&second).expect("second dir");
        let pointer = scratch.path().join("current");
        replace_pointer(&pointer, &first, PointerKind::Directory).expect("create pointer");
        assert_eq!(classify(&pointer).expect("pointer"), EntryKind::Link);
        assert_eq!(std::fs::read_link(&pointer).expect("identity"), first);
        replace_pointer(&pointer, &second, PointerKind::Directory).expect("replace pointer");
        assert_eq!(
            std::fs::read_link(&pointer).expect("new identity"),
            second,
            "the pointer names the new generation"
        );
        assert!(first.is_dir(), "the previous generation survives");
        let foreign = scratch.path().join("foreign");
        std::fs::write(&foreign, b"foreign\n").expect("foreign file");
        let err = replace_pointer(&foreign, &second, PointerKind::Directory)
            .expect_err("foreign state is not adopted");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read(&foreign).expect("foreign intact"),
            b"foreign\n"
        );
        let tree = scratch.path().join("tree");
        std::fs::create_dir(&tree).expect("tree");
        let err = replace_pointer(&tree, &second, PointerKind::Directory)
            .expect_err("a tree is never adopted");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(tree.is_dir(), "the tree survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replaces_file_pointers_when_the_host_allows_links() {
        let scratch = scratch("dx-fs-file-pointer-");
        let tool = scratch.path().join("tool");
        std::fs::write(&tool, b"tool\n").expect("tool");
        let pointer: PathBuf = scratch.path().join("tool.exe");
        if skip_without_link_privileges(file_pointer(&tool, &pointer)) {
            scratch.close().expect("cleanup");
            return;
        }
        assert_eq!(classify(&pointer).expect("pointer"), EntryKind::Link);
        replace_pointer(&pointer, &tool, PointerKind::File).expect("replace pointer");
        assert_eq!(std::fs::read_link(&pointer).expect("identity"), tool);
        assert_eq!(classify(&pointer).expect("still a link"), EntryKind::Link);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replaces_directory_pointers_when_the_host_allows_links() {
        let scratch = scratch("dx-fs-dir-pointer-");
        let first = scratch.path().join("first");
        std::fs::create_dir(&first).expect("first dir");
        let pointer = scratch.path().join("current");
        if skip_without_link_privileges(directory_pointer(&first, &pointer)) {
            scratch.close().expect("cleanup");
            return;
        }
        assert_eq!(classify(&pointer).expect("pointer"), EntryKind::Link);
        remove_managed(&pointer).expect("remove pointer");
        assert!(first.is_dir(), "the directory target survives removal");
        scratch.close().expect("cleanup");
    }
}
