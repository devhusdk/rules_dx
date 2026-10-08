use std::path::Path;

pub use dx_preset::{
    owned_collisions_in_content, preset_paths, render_fragment as render_preset_fragment,
    PRESET_BAZEL_VERSION,
};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PresetError {
    #[error("root .bazelrc duplicates preset lines; reconcile (remove owned duplicates, keep project overrides explicit): {lines}")]
    OwnedCollision { lines: String },
    #[error("preset stale: {detail}")]
    Stale { detail: String },
    #[error("cannot write {path}: {detail}")]
    Unwritable { path: String, detail: String },
}

fn root_collisions(root_path: &Path, rendered: &str) -> Result<Vec<String>, PresetError> {
    match std::fs::read_to_string(root_path) {
        Ok(content) => Ok(owned_collisions_in_content(&content, rendered)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(PresetError::Unwritable {
            path: ".bazelrc".to_owned(),
            detail: error.to_string(),
        }),
    }
}

fn collision_error(collisions: &[String]) -> PresetError {
    PresetError::OwnedCollision {
        lines: collisions.join(", "),
    }
}

fn stale_error(detail: String) -> PresetError {
    PresetError::Stale { detail }
}

pub fn check_preset(workspace: &Path) -> Result<(), PresetError> {
    let (root_path, fragment_path) = preset_paths(workspace);
    let rendered = render_preset_fragment();
    let collisions = root_collisions(&root_path, &rendered)?;
    if !collisions.is_empty() {
        return Err(collision_error(&collisions));
    }
    let checked_in = std::fs::read_to_string(&fragment_path).map_err(|error| {
        stale_error(format!(
            "tools/bazelrc/preset.bazelrc is stale (missing); run `bazel run //tools/bazelrc:preset_update` to regenerate ({error})"
        ))
    })?;
    if checked_in == rendered {
        Ok(())
    } else {
        Err(stale_error(format!(
            "tools/bazelrc/preset.bazelrc is stale; run `bazel run //tools/bazelrc:preset_update` to regenerate\n{}",
            dx_preset::unified_diff(&checked_in, &rendered)
        )))
    }
}

pub fn update_preset(workspace: &Path) -> Result<(), PresetError> {
    let (root_path, fragment_path) = preset_paths(workspace);
    let rendered = render_preset_fragment();
    let collisions = root_collisions(&root_path, &rendered)?;
    if !collisions.is_empty() {
        return Err(collision_error(&collisions));
    }
    if let Some(parent) = fragment_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| PresetError::Unwritable {
            path: "tools/bazelrc/preset.bazelrc".to_owned(),
            detail: error.to_string(),
        })?;
    }
    dx_atomic_fs::write_atomic(&fragment_path, rendered.as_bytes()).map_err(|error| {
        PresetError::Unwritable {
            path: "tools/bazelrc/preset.bazelrc".to_owned(),
            detail: error.to_string(),
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx_stamp_tracks_single_version() {
        assert_eq!(crate::version::DX_VERSION, "0.0.0");
        assert_eq!(PRESET_BAZEL_VERSION, "9.2.0");
    }

    #[test]
    fn check_and_update_round_trip_in_scratch() {
        let scratch = dx_test_scratch::scratch("dx-preset-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("MODULE.bazel"), "").expect("module");
        std::fs::write(
            root.join(".bazelrc"),
            "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
        )
        .expect("bazelrc");
        assert!(matches!(
            check_preset(&root),
            Err(PresetError::Stale { .. })
        ));
        update_preset(&root).expect("update creates fragment");
        assert_eq!(check_preset(&root), Ok(()));
        let (_, fragment) = preset_paths(&root);
        std::fs::write(&fragment, "# dirty\n").expect("dirty");
        match check_preset(&root) {
            Err(PresetError::Stale { detail }) => {
                assert!(detail.contains("stale"));
                assert!(detail.contains("checked-in"));
            }
            other => panic!("want Stale, got {other:?}"),
        }
        update_preset(&root).expect("update fixes dirty");
        assert_eq!(check_preset(&root), Ok(()));
        std::fs::write(
            root.join(".bazelrc"),
            "import %workspace%/tools/bazelrc/preset.bazelrc\ncommon --enable_bzlmod\n",
        )
        .expect("collision");
        assert!(matches!(
            check_preset(&root),
            Err(PresetError::OwnedCollision { .. })
        ));
        assert!(matches!(
            update_preset(&root),
            Err(PresetError::OwnedCollision { .. })
        ));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn unreadable_root_bazelrc_fails_closed() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let scratch = dx_test_scratch::scratch("dx-preset-unreadable-");
            let root = scratch.path().to_path_buf();
            let bazelrc = root.join(".bazelrc");
            std::fs::write(&bazelrc, "common --enable_bzlmod\n").expect("bazelrc");
            let mut permissions = std::fs::metadata(&bazelrc).expect("stat").permissions();
            permissions.set_mode(0o000);
            std::fs::set_permissions(&bazelrc, permissions).expect("chmod");
            let outcome = check_preset(&root);
            std::fs::set_permissions(&bazelrc, std::fs::Permissions::from_mode(0o600))
                .expect("chmod back");
            match outcome {
                Err(PresetError::Unwritable { path, .. }) => assert_eq!(path, ".bazelrc"),
                other => panic!("want Unwritable for an unreadable .bazelrc, got {other:?}"),
            }
            scratch.close().expect("cleanup");
        }
    }
}
