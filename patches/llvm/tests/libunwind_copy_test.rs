//! libunwind.a must be a regular file; a symlink dangles once its target leaves a reused output base (#7781).

#[test]
fn libunwind_archive_is_a_regular_file() {
    let given = dx_testing::env_file("DX_LIBUNWIND");
    let target = match std::fs::read_link(&given) {
        Ok(link) if link.is_absolute() => link,
        Ok(link) => given
            .parent()
            .map_or(link.clone(), |parent| parent.join(&link)),
        Err(_) => given.clone(),
    };
    let metadata = std::fs::symlink_metadata(&target)
        .unwrap_or_else(|error| panic!("{}: {error}", target.display()));
    assert!(
        !metadata.file_type().is_symlink(),
        "{} resolves to a symbolic link",
        given.display()
    );
    assert!(
        metadata.is_file(),
        "{} is not a regular file",
        target.display()
    );
}
