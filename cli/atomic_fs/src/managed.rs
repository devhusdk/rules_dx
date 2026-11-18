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

fn copy_file(source: &Path, destination: &Path) -> io::Result<Mechanism> {
    let input = std::fs::File::open(source)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = tempfile::Builder::new()
        .prefix(&crate::staging_prefix(destination))
        .suffix(crate::STAGING_SUFFIX)
        .tempfile_in(parent)?;
    crate::persist_copy(input, staging, destination, crate::Durability::Unsynced)
        .map(|()| Mechanism::Copied)
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

/// Creates a new link at `pointer` naming `target`; never overwrites an existing entry.
pub fn create_pointer(target: &Path, pointer: &Path, kind: PointerKind) -> io::Result<()> {
    #[cfg(windows)]
    {
        match kind {
            PointerKind::File => std::os::windows::fs::symlink_file(target, pointer),
            PointerKind::Directory => std::os::windows::fs::symlink_dir(target, pointer),
        }
    }
    #[cfg(not(windows))]
    {
        let _: PointerKind = kind;
        std::os::unix::fs::symlink(target, pointer)
    }
}

/// Points `pointer` at `target` by renaming a freshly made sibling link over it.
pub fn replace_pointer(target: &Path, pointer: &Path, kind: PointerKind) -> io::Result<()> {
    let parent = pointer
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(crate::long_path(parent)?)?;
    let ((), stage) = tempfile::Builder::new()
        .prefix(&crate::staging_prefix(pointer))
        .suffix(crate::STAGING_SUFFIX)
        .make_in(crate::long_path(parent)?, |path| {
            create_pointer(target, path, kind)
        })?
        .keep()?;
    std::fs::rename(&stage, pointer).inspect_err(|_| {
        drop(remove_managed(&stage));
    })
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

    fn skip_without_link_privileges(result: &io::Result<()>) -> bool {
        if result.is_ok() {
            return false;
        } // LCOV_EXCL_LINE - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        eprintln!("host refuses links; the copy path covers this case instead");
        true
        // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }

    #[test]
    fn classifies_regular_entries_without_following_links() {
        let scratch = scratch("dx-fs-classify-").unwrap();
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
            classify(scratch.path()).expect("root"),
            EntryKind::Directory
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn classifies_links_and_dangling_links() {
        let scratch = scratch("dx-fs-classify-link-").unwrap();
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
    #[cfg(unix)]
    fn classify_reports_a_path_under_a_file() {
        let scratch = scratch("dx-fs-classify-under-file-").unwrap();
        let file = scratch.path().join("file.txt");
        std::fs::write(&file, b"x\n").expect("file");
        let err = classify(&file.join("child.txt")).expect_err("a file has no children");
        assert_eq!(err.kind(), io::ErrorKind::NotADirectory);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn classify_reports_an_invalid_path() {
        let err = classify(Path::new("bad\0name")).expect_err("a NUL byte is not a path");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn materializes_by_copy_when_the_link_fails() {
        let scratch = scratch("dx-fs-link-fallback-").unwrap();
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("current.toml");
        assert_eq!(
            materialize_file_with(&source, &destination, LinkPolicy::LinkOrCopy, &refuses_link)
                .expect("copy fallback"),
            Mechanism::Copied
        );
        assert_eq!(classify(&destination).expect("copy"), EntryKind::File);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn materializes_bytes_and_reports_the_mechanism() {
        let scratch = scratch("dx-fs-materialize-").unwrap();
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
    fn prefers_a_link_and_keeps_its_identity() {
        let scratch = scratch("dx-fs-materialize-link-").unwrap();
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
        let scratch = scratch("dx-fs-materialize-exec-").unwrap();
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
        let scratch = scratch("dx-fs-require-link-").unwrap();
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
    fn materialize_required_link_keeps_its_identity() {
        let scratch = scratch("dx-fs-require-link-ok-").unwrap();
        let probe_source = scratch.path().join("probe.bin");
        std::fs::write(&probe_source, b"probe\n").expect("probe source");
        let probe_link = scratch.path().join("probe-link");
        if skip_without_link_privileges(&file_pointer(&probe_source, &probe_link)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        remove_managed(&probe_link).expect("remove probe link");
        let source = scratch.path().join("pointer.bin");
        std::fs::write(&source, b"tool\n").expect("source");
        let destination = scratch.path().join("current");
        assert_eq!(
            materialize_file(&source, &destination, LinkPolicy::RequireLink)
                .expect("required link"),
            Mechanism::Linked
        );
        assert_eq!(
            std::fs::read_link(&destination).expect("link identity"),
            source
        );
        assert_eq!(classify(&destination).expect("link"), EntryKind::Link);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replaces_an_existing_destination_with_a_copy() {
        let scratch = scratch("dx-fs-replace-").unwrap();
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
        let scratch = scratch("dx-fs-dir-dest-").unwrap();
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
    fn refuses_a_link_destination_and_keeps_the_target() {
        let scratch = scratch("dx-fs-link-dest-").unwrap();
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let outside = scratch.path().join("outside.toml");
        std::fs::write(&outside, b"secret\n").expect("outside");
        let destination = scratch.path().join("current.toml");
        if skip_without_link_privileges(&file_pointer(&outside, &destination)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
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
        let scratch = scratch("dx-fs-source-").unwrap();
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
        let err = materialize_file(scratch.path(), &destination, LinkPolicy::LinkOrCopy)
            .expect_err("directory source");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn reports_a_missing_destination_parent() {
        let scratch = scratch("dx-fs-parent-").unwrap();
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("absent").join("out.toml");
        for policy in [LinkPolicy::LinkOrCopy, LinkPolicy::RequireLink] {
            let err = materialize_file(&source, &destination, policy).expect_err("no parent");
            assert_eq!(err.kind(), io::ErrorKind::NotFound, "{policy:?}");
        }
        scratch.close().expect("cleanup");
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[cfg(unix)]
    fn chmod(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    }

    #[test]
    #[cfg(unix)]
    fn replaces_a_read_only_destination() {
        let scratch = scratch("dx-fs-readonly-").unwrap();
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let destination = scratch.path().join("locked.toml");
        std::fs::write(&destination, b"stale\n").expect("existing");
        chmod(&destination, 0o444);
        let mechanism = materialize_file(&source, &destination, LinkPolicy::LinkOrCopy)
            .expect("a read-only destination is replaced");
        assert_eq!(mechanism, Mechanism::Copied);
        assert_eq!(
            std::fs::read(&destination).expect("bytes"),
            b"config = true\n"
        );
        assert_eq!(mode_of(&destination), 0o644);
        let mut names: Vec<_> = std::fs::read_dir(scratch.path())
            .expect("list")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["locked.toml", "source.toml"]);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn copied_read_only_sources_can_be_restaged() {
        let scratch = scratch("dx-fs-readonly-source-").unwrap();
        let refuse = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::PermissionDenied));
        for (source_mode, expected) in [(0o444, 0o644), (0o555, 0o755)] {
            let source = scratch.path().join(format!("source-{source_mode:o}"));
            std::fs::write(&source, b"x\n").expect("source");
            chmod(&source, source_mode);
            let destination = scratch.path().join(format!("dest-{source_mode:o}"));
            for _ in 0..2 {
                let mechanism =
                    materialize_file_with(&source, &destination, LinkPolicy::LinkOrCopy, &refuse)
                        .expect("copy");
                assert_eq!(mechanism, Mechanism::Copied);
                assert_eq!(mode_of(&destination), expected);
            }
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn preserves_the_error_of_a_read_only_destination_parent() {
        let scratch = scratch("dx-fs-readonly-parent-").unwrap();
        let source = scratch.path().join("source.toml");
        std::fs::write(&source, b"config = true\n").expect("source");
        let parent = scratch.path().join("locked");
        std::fs::create_dir(&parent).expect("parent");
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555))
                .expect("chmod 555");
        }
        let result = materialize_file(&source, &parent.join("out.toml"), LinkPolicy::LinkOrCopy);
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&parent)
                .expect("parent")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o555, "the parent mode is not forced");
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))
                .expect("chmod back");
        }
        // Root may write into a read-only directory, so for root the file lands.
        if dx_test_scratch::running_as_root() {
            result.expect("root writes through the mode");
        } else {
            let err = result.expect_err("a read-only parent is not forced");
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn removes_a_file_and_reports_missing_paths_and_trees() {
        let scratch = scratch("dx-fs-remove-").unwrap();
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
        let scratch = scratch("dx-fs-remove-link-").unwrap();
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
    fn replaces_directory_pointers_when_the_host_allows_links() {
        let scratch = scratch("dx-fs-dir-pointer-").unwrap();
        let first = scratch.path().join("first");
        std::fs::create_dir(&first).expect("first dir");
        let pointer = scratch.path().join("current");
        if skip_without_link_privileges(&directory_pointer(&first, &pointer)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        assert_eq!(classify(&pointer).expect("pointer"), EntryKind::Link);
        remove_managed(&pointer).expect("remove pointer");
        assert!(first.is_dir(), "the directory target survives removal");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn create_pointer_links_a_file() {
        let scratch = scratch("dx-fs-file-pointer-").unwrap();
        let target = scratch.path().join("target.txt");
        std::fs::write(&target, b"target\n").expect("target");
        let link = scratch.path().join("link");
        if skip_without_link_privileges(&create_pointer(&target, &link, PointerKind::File)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        assert_eq!(classify(&link).expect("link"), EntryKind::Link);
        assert_eq!(std::fs::read_link(&link).expect("identity"), target);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn create_pointer_refuses_an_existing_entry() {
        let scratch = scratch("dx-fs-create-pointer-").unwrap();
        let target = scratch.path().join("target");
        std::fs::create_dir(&target).expect("target dir");
        let other = scratch.path().join("other");
        std::fs::create_dir(&other).expect("other dir");
        let link = scratch.path().join("link");
        if skip_without_link_privileges(&directory_pointer(&target, &link)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        let file = scratch.path().join("file");
        std::fs::write(&file, b"file\n").expect("file");
        create_pointer(&other, &file, PointerKind::Directory)
            .expect_err("an existing file is not overwritten");
        assert_eq!(std::fs::read(&file).expect("file intact"), b"file\n");
        create_pointer(&other, &link, PointerKind::Directory)
            .expect_err("an existing link is not overwritten");
        assert_eq!(std::fs::read_link(&link).expect("link intact"), target);
        scratch.close().expect("cleanup");
    }

    fn pointer_siblings(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("list")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn replace_pointer_swaps_file_and_directory_links() {
        let scratch = scratch("dx-fs-replace-pointer-").unwrap();
        let targets = scratch.path().join("targets");
        std::fs::create_dir(&targets).expect("targets");
        let first = targets.join("first.txt");
        let second = targets.join("second.txt");
        std::fs::write(&first, b"first\n").expect("first");
        std::fs::write(&second, b"second\n").expect("second");
        let first_dir = targets.join("first");
        let second_dir = targets.join("second");
        std::fs::create_dir(&first_dir).expect("first dir");
        std::fs::create_dir(&second_dir).expect("second dir");
        let links = scratch.path().join("links");
        let file_link = links.join("file");
        if skip_without_link_privileges(&replace_pointer(&first, &file_link, PointerKind::File)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        replace_pointer(&second, &file_link, PointerKind::File).expect("replace file link");
        assert_eq!(std::fs::read_link(&file_link).expect("file link"), second);
        let dir_link = links.join("dir");
        replace_pointer(&first_dir, &dir_link, PointerKind::Directory).expect("dir link");
        replace_pointer(&second_dir, &dir_link, PointerKind::Directory).expect("replace dir link");
        assert_eq!(std::fs::read_link(&dir_link).expect("dir link"), second_dir);
        assert!(first_dir.is_dir(), "the old target survives");
        assert_eq!(pointer_siblings(&links), ["dir", "file"]);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn replace_pointer_keeps_a_real_directory_and_drops_the_stage() {
        let scratch = scratch("dx-fs-replace-pointer-dir-").unwrap();
        let target = scratch.path().join("target");
        std::fs::create_dir(&target).expect("target");
        let links = scratch.path().join("links");
        let pointer = links.join("dir");
        std::fs::create_dir_all(&pointer).expect("real dir");
        std::fs::write(pointer.join("keep"), b"keep\n").expect("keep");
        let probe = links.join("probe");
        if skip_without_link_privileges(&create_pointer(&target, &probe, PointerKind::Directory)) {
            // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            scratch.close().expect("cleanup");
            return;
            // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        remove_managed(&probe).expect("drop probe");
        replace_pointer(&target, &pointer, PointerKind::Directory)
            .expect_err("a non-empty directory is not replaced");
        assert_eq!(
            std::fs::read(pointer.join("keep")).expect("kept"),
            b"keep\n"
        );
        assert_eq!(pointer_siblings(&links), ["dir"]);
        scratch.close().expect("cleanup");
    }
}
