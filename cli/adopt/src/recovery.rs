use std::path::{Path, PathBuf};

pub const RECOVERY_DIR_NAME: &str = "recovery";
pub const CACHE_DIR_NAME: &str = "cache";
pub const VERSION_PIN_RECORD: &str = "version-pin.toml";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRecord {
    pub operation: String,
    pub complete: bool,
    pub previous: String,
    pub current: String,
    pub rollback: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryItem {
    pub name: String,
    pub path: PathBuf,
    pub record: RecoveryRecord,
}

pub fn recovery_dir(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join(RECOVERY_DIR_NAME)
}

pub fn cache_dir(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join(CACHE_DIR_NAME)
}

fn read_record(path: &Path) -> Result<RecoveryRecord, super::AdoptError> {
    let text = std::fs::read_to_string(path).map_err(|e| super::AdoptError::RecordRecovery {
        detail: format!("cannot read {}: {e}", path.display()),
    })?;
    toml::from_str(&text).map_err(|e| super::AdoptError::RecordRecovery {
        detail: format!("{}: invalid recovery record: {e}", path.display()),
    })
}

pub fn list_recovery(workspace: &Path) -> Result<Vec<RecoveryItem>, super::AdoptError> {
    let dir = recovery_dir(workspace);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(super::AdoptError::RecordRecovery {
                detail: format!("cannot list {}: {e}", dir.display()),
            });
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| super::AdoptError::RecordRecovery {
            detail: format!("cannot list {}: {e}", dir.display()),
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".toml") {
            names.push(name);
        }
    }
    names.sort();
    let mut items = Vec::with_capacity(names.len());
    for name in names {
        let path = dir.join(&name);
        let record = read_record(&path)?;
        items.push(RecoveryItem { name, path, record });
    }
    Ok(items)
}

pub fn record_previous_pin(
    workspace: &Path,
    previous: &str,
    current: &str,
) -> Result<PathBuf, super::AdoptError> {
    let dir = recovery_dir(workspace);
    std::fs::create_dir_all(&dir).map_err(|e| super::AdoptError::RecordRecovery {
        detail: format!("cannot create {}: {e}", dir.display()),
    })?;
    let record = RecoveryRecord {
        operation: "version-pin".to_owned(),
        complete: true,
        previous: previous.to_owned(),
        current: current.to_owned(),
        rollback: format!("dx version --apply --pin {previous}"),
    };
    let body = format!(
        "operation = {:?}\ncomplete = true\nprevious = {:?}\ncurrent = {:?}\nrollback = {:?}\n",
        record.operation, record.previous, record.current, record.rollback
    );
    let path = dir.join(VERSION_PIN_RECORD);
    dx_atomic_fs::write_atomic(&path, body.as_bytes()).map_err(|e| {
        super::AdoptError::RecordRecovery {
            detail: format!("cannot write {}: {e}", path.display()),
        }
    })?;
    Ok(path)
}

pub fn describe_record(item: &RecoveryItem) -> String {
    let record = &item.record;
    let state = if record.complete {
        "complete"
    } else {
        "incomplete"
    };
    format!(
        "recovery record {}: {} {} -> {} ({state}; rollback `{}`; deleting loses rollback to {})",
        item.name,
        record.operation,
        record.previous,
        record.current,
        record.rollback,
        record.previous
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryDeletion {
    pub deletable: Vec<RecoveryItem>,
    pub refused: Vec<RecoveryItem>,
}

pub fn plan_recovery_deletion(items: &[RecoveryItem]) -> RecoveryDeletion {
    let mut deletion = RecoveryDeletion {
        deletable: Vec::new(),
        refused: Vec::new(),
    };
    for item in items {
        if item.record.complete {
            deletion.deletable.push(item.clone());
        } else {
            deletion.refused.push(item.clone());
        }
    }
    deletion
}

pub fn apply_recovery_deletion(
    deletion: &RecoveryDeletion,
) -> Result<Vec<PathBuf>, super::AdoptError> {
    let mut removed = Vec::with_capacity(deletion.deletable.len());
    for item in &deletion.deletable {
        std::fs::remove_file(&item.path).map_err(|e| super::AdoptError::RecordRecovery {
            detail: format!("cannot remove {}: {e}", item.path.display()),
        })?;
        removed.push(item.path.clone());
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> (dx_test_scratch::TempDir, PathBuf) {
        let scratch = dx_test_scratch::scratch(name);
        let root = scratch.path().to_path_buf();
        (scratch, root)
    }

    #[test]
    fn missing_recovery_dir_lists_empty() {
        let (scratch, root) = scratch("dx-recovery-missing-");
        assert!(list_recovery(&root).expect("missing lists empty").is_empty());
        assert!(!recovery_dir(&root).exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn pin_displacement_round_trips_through_toml() {
        let (scratch, root) = scratch("dx-recovery-pin-");
        let path = record_previous_pin(&root, "1.2.3", "2.0.0").expect("records");
        assert_eq!(path, recovery_dir(&root).join(VERSION_PIN_RECORD));
        let items = list_recovery(&root).expect("lists");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, VERSION_PIN_RECORD);
        assert_eq!(
            items[0].record,
            RecoveryRecord {
                operation: "version-pin".to_owned(),
                complete: true,
                previous: "1.2.3".to_owned(),
                current: "2.0.0".to_owned(),
                rollback: "dx version --apply --pin 1.2.3".to_owned(),
            }
        );
        let described = describe_record(&items[0]);
        assert!(described.contains(VERSION_PIN_RECORD), "{described}");
        assert!(described.contains("1.2.3 -> 2.0.0"), "{described}");
        assert!(described.contains("dx version --apply --pin 1.2.3"), "{described}");
        assert!(described.contains("loses rollback to 1.2.3"), "{described}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn malformed_record_fails_closed_and_non_toml_is_ignored() {
        let (scratch, root) = scratch("dx-recovery-malformed-");
        std::fs::create_dir_all(recovery_dir(&root)).expect("recovery dir");
        std::fs::write(recovery_dir(&root).join("notes.txt"), "not a record").expect("notes");
        assert!(list_recovery(&root).expect("txt ignored").is_empty());
        std::fs::write(recovery_dir(&root).join("bad.toml"), "not toml = [").expect("bad");
        let error = list_recovery(&root).expect_err("malformed fails");
        assert!(error.to_string().contains("bad.toml"), "{error}");
        std::fs::remove_file(recovery_dir(&root).join("bad.toml")).expect("remove bad");
        std::fs::write(
            recovery_dir(&root).join("unknown.toml"),
            "operation = \"x\"\ncomplete = true\nprevious = \"a\"\ncurrent = \"b\"\nrollback = \"c\"\nextra = 1\n",
        )
        .expect("unknown");
        let error = list_recovery(&root).expect_err("unknown field fails");
        assert!(error.to_string().contains("unknown.toml"), "{error}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn incomplete_records_are_refused_while_complete_ones_delete() {
        let (scratch, root) = scratch("dx-recovery-delete-");
        record_previous_pin(&root, "1.2.3", "2.0.0").expect("complete record");
        std::fs::write(
            recovery_dir(&root).join("active.toml"),
            "operation = \"upgrade\"\ncomplete = false\nprevious = \"1.2.3\"\ncurrent = \"2.0.0\"\nrollback = \"dx upgrade --from 1.2.3 --to 2.0.0\"\n",
        )
        .expect("incomplete record");
        let items = list_recovery(&root).expect("lists both");
        assert_eq!(items.len(), 2);
        let deletion = plan_recovery_deletion(&items);
        assert_eq!(deletion.deletable.len(), 1);
        assert_eq!(deletion.refused.len(), 1);
        assert_eq!(deletion.refused[0].name, "active.toml");
        let removed = apply_recovery_deletion(&deletion).expect("applies");
        assert_eq!(removed.len(), 1);
        assert!(!recovery_dir(&root).join(VERSION_PIN_RECORD).exists());
        assert!(recovery_dir(&root).join("active.toml").exists());
        scratch.close().expect("cleanup");
    }
}
