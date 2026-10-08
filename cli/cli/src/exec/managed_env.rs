use super::common::*;
use super::managed_staging::{ensure_generation_dir, symlink_leaf};
use dx_atomic_fs::lease::SharedLease;
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) type ManagedEnvCollection = (Vec<dx_bep::TargetOutput>, dx_env_plan::CollectedPlan);

pub(crate) fn collect_managed_env(
    bep: &Path,
    workspace: &Path,
) -> Result<ManagedEnvCollection, (String, String)> {
    let outputs = collect_targets(bep, dx_env_plan::OUTPUT_GROUP, workspace)?;
    let plan = dx_env_plan::collect_plan(&outputs).map_err(|err| {
        (
            CODE_INVALID_RESULT.to_owned(),
            format!("invalid env plan: {err}"),
        )
    })?;
    Ok((outputs, plan))
}

pub(crate) fn empty_env_id() -> dx_setup::GenerationId {
    dx_setup::GenerationId::from_digest(dx_env_plan::plan_digest("[]"))
}

fn validate_env_key(key: &str) -> Result<(), ExecError> {
    if key.is_empty() {
        return Err(ExecError::EmptyEnvKey);
    }
    if key.contains('/') || key.contains('\\') || key == "." || key == ".." {
        return Err(ExecError::BadEnvKey {
            key: key.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn stage_env_generation(
    state_root: &Path,
    id: &dx_setup::GenerationId,
    projection: &[dx_env_plan::ProjectionEntry],
) -> Result<SharedLease, (String, String)> {
    let (dir, lease) =
        ensure_generation_dir(state_root, dx_setup::ENVIRONMENTS_DIR_NAME, id.as_str())?;
    let mut seen: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
    for entry in projection {
        validate_env_key(&entry.key).map_err(|reason| {
            (
                CODE_INVALID_RESULT.to_owned(),
                format!("invalid env plan: {reason}"),
            )
        })?;
        if let Some((value, artifact)) = seen.insert(
            entry.key.as_str(),
            (entry.value.as_str(), entry.artifact.as_str()),
        ) {
            if value != entry.value.as_str() || artifact != entry.artifact.as_str() {
                return Err((
                    CODE_INVALID_RESULT.to_owned(),
                    format!("env identity key {:?} maps to multiple inputs", entry.key),
                ));
            }
        }
    }
    let artifacts = dir.join("artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| {
        (
            CODE_MANAGED_COMMIT_FAILED.to_owned(),
            format!("cannot create {}: {e}", artifacts.display()),
        )
    })?;
    for entry in projection {
        let artifact = Path::new(&entry.artifact);
        if artifact.symlink_metadata().is_err() {
            return Err((
                CODE_INVALID_RESULT.to_owned(),
                format!(
                    "invalid env plan: referenced artifact {:?} is missing; Bazel owns materialization",
                    entry.artifact
                ),
            ));
        }
        let leaf = artifacts.join(&entry.key);
        let needs_link = match std::fs::symlink_metadata(&leaf) {
            Ok(meta) => {
                if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
                    return Err((
                        CODE_INVALID_RESULT.to_owned(),
                        format!(
                            "invalid env plan: env identity key {:?} collides within its generation",
                            entry.key
                        ),
                    ));
                }
                match std::fs::read_link(&leaf) {
                    Ok(current) if current == *artifact => false,
                    _ => {
                        std::fs::remove_file(&leaf).map_err(|e| {
                            (
                                CODE_MANAGED_COMMIT_FAILED.to_owned(),
                                format!("cannot replace {}: {e}", leaf.display()),
                            )
                        })?;
                        true
                    }
                }
            }
            Err(_) => true,
        };
        if needs_link {
            symlink_leaf(artifact, &leaf).map_err(|e| {
                (
                    CODE_MANAGED_COMMIT_FAILED.to_owned(),
                    format!("cannot link {}: {e}", leaf.display()),
                )
            })?;
        }
    }
    let values: BTreeMap<&str, &str> = seen
        .iter()
        .map(|(key, (value, _))| (*key, *value))
        .collect();
    let rendered = serde_json::to_string(&values).map_err(|e| {
        (
            CODE_MANAGED_COMMIT_FAILED.to_owned(),
            format!("cannot render values.json: {e}"),
        )
    })?;
    let target = dir.join("values.json");
    dx_atomic_fs::write_atomic(&target, rendered.as_bytes()).map_err(|e| {
        (
            CODE_MANAGED_COMMIT_FAILED.to_owned(),
            format!("cannot publish {}: {e}", target.display()),
        )
    })?;
    Ok(lease)
}

pub(crate) fn stage_env_side(
    state_root: &Path,
    plan: &dx_env_plan::CollectedPlan,
) -> Result<(dx_setup::GenerationId, SharedLease), (String, String)> {
    let id = dx_setup::GenerationId::from_digest(plan.digest);
    let lease = stage_env_generation(state_root, &id, &plan.projection)?;
    Ok((id, lease))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use dx_setup::{read_current_pair, ENVIRONMENTS_DIR_NAME};

    #[test]
    fn managed_live_invalid_env_shard_fails_closed() {
        let mut harness = Harness::new("managed-bad-env-shard");
        let shard = harness.temp.join("bad.dxenv.pb");
        std::fs::write(&shard, b"not an env shard").expect("shard");
        harness.raw_bep = Some(managed_shard_bep(&shard, dx_env_plan::OUTPUT_GROUP));
        let (code, _, err) = harness.run(&["env"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("dx: invalid_result: invalid env plan"),
            "{err}"
        );
        assert_eq!(
            read_current_pair(&harness.workspace).expect("read current"),
            None,
            "collection failure commits nothing"
        );
    }

    #[test]
    fn managed_env_keys_validate() {
        assert!(validate_env_key("key.json").is_ok());
        for bad in ["", "a/b", "a\\b", ".", ".."] {
            assert!(validate_env_key(bad).is_err(), "env key {bad:?} must fail");
        }
    }

    #[test]
    fn staged_environment_stays_leased_until_released() {
        let fixture = managed_stage_fixture("managed-env-leased");
        let workspace = fixture.workspace().to_path_buf();
        let first = fixture.first.clone();
        let id = empty_env_id();
        let lease =
            stage_env_generation(&workspace, &id, &[env_entry("k", "v", &first)]).expect("stage");
        assert!(
            dx_atomic_fs::lease::shared_held(
                &workspace.join(".dx"),
                dx_atomic_fs::lease::GenerationUse::Environment,
                id.as_str(),
            )
            .expect("probe"),
            "staging holds the generation lease so cleanup cannot prune mid-stage"
        );
        drop(lease);
        assert!(
            !dx_atomic_fs::lease::shared_held(
                &workspace.join(".dx"),
                dx_atomic_fs::lease::GenerationUse::Environment,
                id.as_str(),
            )
            .expect("reprobe"),
            "releasing the staging lease makes the generation prunable again"
        );
    }

    #[test]
    fn managed_stage_env_mirrors_leaves_and_values() {
        let fixture = managed_stage_fixture("managed-env-mirror");
        let workspace = fixture.workspace().to_path_buf();
        let first = fixture.first.clone();
        let second = fixture.second.clone();
        let id = empty_env_id();
        let projection = vec![
            env_entry("k2", "x\"y", &second),
            env_entry("k1", "v1", &first),
        ];
        stage_env_generation(&workspace, &id, &projection).expect("stage");
        let dir = workspace
            .join(".dx")
            .join(ENVIRONMENTS_DIR_NAME)
            .join(id.as_str());
        assert_eq!(
            std::fs::read_link(dir.join("artifacts/k1")).expect("leaf"),
            first
        );
        assert_eq!(
            std::fs::read_link(dir.join("artifacts/k2")).expect("leaf"),
            second
        );
        let values = std::fs::read_to_string(dir.join("values.json")).expect("values");
        assert_eq!(values, "{\"k1\":\"v1\",\"k2\":\"x\\\"y\"}");
        assert!(
            !dir.join("values.json.next").exists(),
            "staging file is always published"
        );
        stage_env_generation(&workspace, &id, &projection).expect("restage");
        assert_eq!(
            std::fs::read_link(dir.join("artifacts/k1")).expect("leaf"),
            first
        );
        std::fs::remove_file(dir.join("artifacts/k1")).expect("remove leaf");
        symlink_leaf(&second, &dir.join("artifacts/k1")).expect("stale leaf");
        stage_env_generation(&workspace, &id, &projection).expect("repair stale");
        assert_eq!(
            std::fs::read_link(dir.join("artifacts/k1")).expect("leaf"),
            first
        );
        let doubled = vec![env_entry("k1", "v1", &first), env_entry("k1", "v1", &first)];
        stage_env_generation(&workspace, &id, &doubled).expect("identical duplicates");
    }

    #[test]
    fn managed_stage_env_rejects_bad_plans() {
        let fixture = managed_stage_fixture("managed-env-reject");
        let workspace = fixture.workspace().to_path_buf();
        let first = fixture.first.clone();
        let second = fixture.second.clone();
        let id = empty_env_id();
        for key in ["", "a/b", ".", ".."] {
            let (code, message) =
                stage_env_generation(&workspace, &id, &[env_entry(key, "v", &first)])
                    .expect_err("bad key");
            assert_eq!(code, CODE_INVALID_RESULT);
            assert!(message.contains("invalid env plan"), "{message}");
        }
        for projection in [
            vec![env_entry("k", "v1", &first), env_entry("k", "v2", &first)],
            vec![env_entry("k", "v", &first), env_entry("k", "v", &second)],
        ] {
            let (code, message) =
                stage_env_generation(&workspace, &id, &projection).expect_err("conflict");
            assert_eq!(code, CODE_INVALID_RESULT);
            assert!(message.contains("multiple inputs"), "{message}");
        }
        let missing = workspace.join("no-such-artifact.txt");
        let (code, message) =
            stage_env_generation(&workspace, &id, &[env_entry("k", "v", &missing)])
                .expect_err("missing artifact");
        assert_eq!(code, CODE_INVALID_RESULT);
        assert!(message.contains("Bazel owns materialization"), "{message}");
        let dir_id = dx_setup::GenerationId::new(&"4".repeat(64)).expect("fixture id");
        let (dir, _lease) =
            ensure_generation_dir(&workspace, ENVIRONMENTS_DIR_NAME, dir_id.as_str())
                .expect("gen dir");
        std::fs::create_dir_all(dir.join("artifacts")).expect("artifacts dir");
        std::fs::create_dir_all(dir.join("artifacts/k")).expect("blocking dir");
        let (code, message) =
            stage_env_generation(&workspace, &dir_id, &[env_entry("k", "v", &first)])
                .expect_err("generation collision");
        assert_eq!(code, CODE_INVALID_RESULT);
        assert!(
            message.contains("collides within its generation"),
            "{message}"
        );
        let blocked_id = dx_setup::GenerationId::new(&"5".repeat(64)).expect("fixture id");
        let (blocked_dir, _held) =
            ensure_generation_dir(&workspace, ENVIRONMENTS_DIR_NAME, blocked_id.as_str())
                .expect("gen dir");
        std::fs::write(blocked_dir.join("artifacts"), "file").expect("blocking file");
        let (code, message) =
            stage_env_generation(&workspace, &blocked_id, &[env_entry("k", "v", &first)])
                .expect_err("artifacts creation");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("cannot create"), "{message}");
        let staging_id = dx_setup::GenerationId::new(&"6".repeat(64)).expect("fixture id");
        let (staging_dir, _staged) =
            ensure_generation_dir(&workspace, ENVIRONMENTS_DIR_NAME, staging_id.as_str())
                .expect("gen dir");
        std::fs::create_dir_all(staging_dir.join("values.json.next")).expect("blocking dir");
        stage_env_generation(&workspace, &staging_id, &[env_entry("k", "v", &first)])
            .expect("legacy staging leftover ignored");
        let publish_id = dx_setup::GenerationId::new(&"7".repeat(64)).expect("fixture id");
        let (publish_dir, _published) =
            ensure_generation_dir(&workspace, ENVIRONMENTS_DIR_NAME, publish_id.as_str())
                .expect("gen dir");
        std::fs::create_dir_all(publish_dir.join("values.json")).expect("blocking dir");
        let (code, message) =
            stage_env_generation(&workspace, &publish_id, &[env_entry("k", "v", &first)])
                .expect_err("values publish");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("cannot publish"), "{message}");
    }

    #[test]
    #[cfg(unix)]
    fn managed_stage_env_filesystem_failures_fail_closed() {
        let fixture = managed_stage_fixture("managed-env-fs");
        let workspace = fixture.workspace().to_path_buf();
        let first = fixture.first.clone();
        let second = fixture.second.clone();
        let id = empty_env_id();
        stage_env_generation(&workspace, &id, &[env_entry("k", "v", &first)]).expect("stage");
        let dir = workspace
            .join(".dx")
            .join(ENVIRONMENTS_DIR_NAME)
            .join(id.as_str());
        set_mode(&dir.join("artifacts"), 0o555);
        let (code, message) =
            stage_env_generation(&workspace, &id, &[env_entry("k", "v", &second)])
                .expect_err("cannot replace");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("cannot replace"), "{message}");
        let (code, message) =
            stage_env_generation(&workspace, &id, &[env_entry("fresh", "v", &second)])
                .expect_err("cannot link");
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
        assert!(message.contains("cannot link"), "{message}");
        set_mode(&dir.join("artifacts"), 0o755);
    }
}
