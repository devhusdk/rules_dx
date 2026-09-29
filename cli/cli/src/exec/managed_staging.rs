use super::common::*;
use dx_bep::CollectorConfig;
use dx_env::DX_DIR_NAME;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub(crate) fn collect_managed_group(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<Vec<dx_bep::TargetOutput>, (String, String)> {
    let file = std::fs::File::open(bep).map_err(|err| {
        (
            CODE_UNREADABLE_BEP.to_owned(),
            format!("failed to read build events: {err}"),
        )
    })?;
    let config = CollectorConfig::new(group).map_err(|err| {
        (
            CODE_INVALID_BEP.to_owned(),
            format!("invalid BEP config: {err}"),
        )
    })?;
    dx_bep::collect_with_workspace(BufReader::new(file), &config, &FsArtifacts, Some(workspace))
        .map_err(|err| {
            (
                CODE_INVALID_BEP.to_owned(),
                format!("invalid build events: {err}"),
            )
        })
}

pub(crate) fn ensure_generation_dir(
    workspace: &Path,
    dir_name: &str,
    hex: &str,
) -> Result<PathBuf, (String, String)> {
    if !workspace.is_dir() {
        return Err((
            CODE_MANAGED_COMMIT_FAILED.to_owned(),
            format!("workspace root {} is not a directory", workspace.display()),
        ));
    }
    let dir = workspace.join(DX_DIR_NAME).join(dir_name).join(hex);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) => {
            if !meta.file_type().is_dir() || meta.file_type().is_symlink() {
                return Err((
                    CODE_MANAGED_COMMIT_FAILED.to_owned(),
                    format!(
                        "{} is not a managed generation directory; refusing to adopt foreign state",
                        dir.display()
                    ),
                ));
            }
        }
        Err(_) => {
            std::fs::create_dir_all(&dir).map_err(|e| {
                (
                    CODE_MANAGED_COMMIT_FAILED.to_owned(),
                    format!("cannot create {}: {e}", dir.display()),
                )
            })?;
        }
    }
    Ok(dir)
}

pub(crate) fn symlink_leaf(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use dx_setup::GENERATED_DIR_NAME;

    #[test]
    fn managed_group_config_failure_is_operational() {
        let harness = Harness::new("managed-bad-group");
        let bep = harness.temp.join("empty.json");
        std::fs::write(&bep, "").expect("bep");
        let (code, message) =
            collect_managed_group(&bep, "", &harness.workspace).expect_err("empty group");
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(message.contains("invalid BEP config"), "{message}");
    }

    #[test]
    fn managed_generation_dir_guards_foreign_state() {
        let workspace = temp_dir("managed-gendir-ws");
        let workspace = workspace.path();
        let hex = "ab".repeat(32);
        let first = ensure_generation_dir(&workspace, GENERATED_DIR_NAME, &hex).expect("create");
        assert!(first.is_dir());
        let second = ensure_generation_dir(&workspace, GENERATED_DIR_NAME, &hex).expect("reuse");
        assert_eq!(first, second);
        let other = "0".repeat(64);
        std::fs::write(
            workspace.join(".dx").join(GENERATED_DIR_NAME).join(&other),
            "foreign",
        )
        .expect("foreign file");
        let (code, message) =
            ensure_generation_dir(&workspace, GENERATED_DIR_NAME, &other).expect_err("refuse");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(
            message.contains("not a managed generation directory"),
            "{message}"
        );
        let file_workspace = workspace.join("ws-file");
        std::fs::write(&file_workspace, "not a dir").expect("workspace file");
        let (code, message) = ensure_generation_dir(&file_workspace, GENERATED_DIR_NAME, &other)
            .expect_err("workspace file");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("is not a directory"), "{message}");
    }

    #[test]
    fn managed_generation_dir_create_failure_surfaces() {
        let workspace = temp_dir("managed-gendir-create-ws");
        let workspace = workspace.path();
        std::fs::create_dir_all(workspace.join(".dx")).expect("dx dir");
        std::fs::write(workspace.join(".dx").join(GENERATED_DIR_NAME), "file").expect("blocker");
        let (code, message) =
            ensure_generation_dir(&workspace, GENERATED_DIR_NAME, &"1".repeat(64))
                .expect_err("create fails");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("cannot create"), "{message}");
    }
}
