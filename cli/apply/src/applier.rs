use std::io;
use std::path::Path;

use dx_digest::sha256_hex;

use super::envelope::Envelope;
use super::validators::{validate, ValidationError};

pub trait FileSystem {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>>;
    fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()>;
}

pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        dx_atomic_fs::write_atomic(path, content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct HookError {
    message: String,
}

impl HookError {
    pub fn new(message: String) -> Self {
        Self { message }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for HookError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

pub trait HookRunner {
    fn run(&self, path: &Path) -> Result<(), HookError>;
}

pub struct NoHooks;

impl HookRunner for NoHooks {
    fn run(&self, _path: &Path) -> Result<(), HookError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub applied: Vec<AppliedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    #[error("validation failed for {path}: {cause}")]
    Validation {
        path: String,
        cause: ValidationError,
    },
    #[error("I/O failed for {path}: {message}")]
    Io { path: String, message: String },
    #[error("hook failed for {path}: {message}")]
    Hook { path: String, message: String },
}

pub fn apply_envelope(
    envelope: &Envelope,
    workspace_root: &Path,
    fs: &dyn FileSystem,
    hooks: &dyn HookRunner,
) -> Result<ApplyReport, ApplyError> {
    let mut applied = Vec::with_capacity(envelope.operations.len());
    for op in &envelope.operations {
        let path = workspace_root.join(&op.path);
        let existing = fs.read(&path).map_err(|err| ApplyError::Io {
            path: op.path.clone(),
            message: err.to_string(),
        })?;
        validate(op, existing.as_deref()).map_err(|cause| ApplyError::Validation {
            path: op.path.clone(),
            cause,
        })?;
        fs.write_atomic(&path, op.content.as_bytes())
            .map_err(|err| ApplyError::Io {
                path: op.path.clone(),
                message: err.to_string(),
            })?;
        hooks.run(&path).map_err(|error| ApplyError::Hook {
            path: op.path.clone(),
            message: error.message().to_owned(),
        })?;
        applied.push(AppliedFile {
            path: op.path.clone(),
            sha256: sha256_hex(op.content.as_bytes()),
        });
    }
    Ok(ApplyReport { applied })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    use super::super::envelope::{FileOperation, ENVELOPE_VERSION};
    use super::*;

    struct FakeFs {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    }

    impl FakeFs {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
            }
        }

        fn files(&self) -> MutexGuard<'_, HashMap<PathBuf, Vec<u8>>> {
            self.files.lock().expect("fake fs lock")
        }
    }

    impl FileSystem for FakeFs {
        fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
            Ok(self.files().get(path).cloned())
        }

        fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()> {
            self.files().insert(path.to_owned(), content.to_vec());
            Ok(())
        }
    }

    struct FailHooks;

    impl HookRunner for FailHooks {
        fn run(&self, _path: &Path) -> Result<(), HookError> {
            Err(HookError::new("hook exploded".to_owned()))
        }
    }

    struct FlakyFs {
        fail_read: bool,
        fail_write: bool,
    }

    impl FileSystem for FlakyFs {
        fn read(&self, _path: &Path) -> io::Result<Option<Vec<u8>>> {
            if self.fail_read {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
            } else {
                Ok(None)
            }
        }

        fn write_atomic(&self, _path: &Path, _content: &[u8]) -> io::Result<()> {
            if self.fail_write {
                Err(io::Error::new(
                    io::ErrorKind::ReadOnlyFilesystem,
                    "read-only",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn create_envelope() -> Envelope {
        Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![FileOperation {
                path: "new.txt".to_owned(),
                original_sha256: None,
                content: "hi\n".to_owned(),
            }],
        }
    }

    fn root() -> PathBuf {
        PathBuf::from("/ws")
    }

    fn update(content: &str) -> FileOperation {
        FileOperation {
            path: "a.txt".to_owned(),
            original_sha256: Some(sha256_hex(b"old\n")),
            content: content.to_owned(),
        }
    }

    #[test]
    fn applies_update_and_reports_digest() {
        let fs = FakeFs::new();
        fs.files().insert(root().join("a.txt"), b"old\n".to_vec());
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![update("new\n")],
        };
        let report = apply_envelope(&envelope, &root(), &fs, &NoHooks).expect("apply");
        assert_eq!(
            report.applied,
            vec![AppliedFile {
                path: "a.txt".to_owned(),
                sha256: sha256_hex(b"new\n"),
            }]
        );
        assert_eq!(
            fs.files().get(&root().join("a.txt")),
            Some(&b"new\n".to_vec())
        );
    }

    #[test]
    fn applies_create() {
        let fs = FakeFs::new();
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![FileOperation {
                path: "sub/new.txt".to_owned(),
                original_sha256: None,
                content: "hi\n".to_owned(),
            }],
        };
        let report = apply_envelope(&envelope, &root(), &fs, &NoHooks).expect("apply");
        assert_eq!(report.applied.len(), 1);
        assert_eq!(
            fs.files().get(&root().join("sub/new.txt")),
            Some(&b"hi\n".to_vec())
        );
    }

    #[test]
    fn validation_failure_aborts_before_write() {
        let fs = FakeFs::new();
        fs.files()
            .insert(root().join("a.txt"), b"changed\n".to_vec());
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![update("new\n")],
        };
        let err = apply_envelope(&envelope, &root(), &fs, &NoHooks).expect_err("stale digest");
        assert_eq!(
            err,
            ApplyError::Validation {
                path: "a.txt".to_owned(),
                cause: ValidationError::DigestMismatch {
                    path: "a.txt".to_owned()
                },
            }
        );
        assert_eq!(
            fs.files().get(&root().join("a.txt")),
            Some(&b"changed\n".to_vec())
        );
    }

    #[test]
    fn hook_failure_aborts_with_hook_error() {
        let fs = FakeFs::new();
        fs.files().insert(root().join("a.txt"), b"old\n".to_vec());
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![update("new\n")],
        };
        let err = apply_envelope(&envelope, &root(), &fs, &FailHooks).expect_err("hook must fail");
        assert_eq!(
            err,
            ApplyError::Hook {
                path: "a.txt".to_owned(),
                message: "hook exploded".to_owned(),
            }
        );
        assert_eq!(
            HookError::new("hook exploded".to_owned()).to_string(),
            "hook exploded"
        );
        assert_eq!(
            HookError::from("hook exploded".to_owned()).message(),
            "hook exploded"
        );
    }

    #[test]
    fn real_filesystem_reads_writes_and_reports_errors() {
        let scratch = dx_test_scratch::scratch("dx-apply-fs-");
        let dir = scratch.path().to_path_buf();
        let fs = RealFileSystem;
        let nested = dir.join("sub").join("a.txt");
        assert_eq!(fs.read(&nested).expect("missing reads as none"), None);
        fs.write_atomic(&nested, b"hello\n").expect("write");
        assert_eq!(
            fs.read(&nested).expect("read back"),
            Some(b"hello\n".to_vec())
        );
        let entries: Vec<_> = std::fs::read_dir(dir.join("sub"))
            .expect("list target dir")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(entries, vec!["a.txt".to_owned()]);
        fs.write_atomic(&nested, b"updated\n").expect("overwrite");
        assert_eq!(
            fs.read(&nested).expect("read overwrite"),
            Some(b"updated\n".to_vec())
        );
        let entries: Vec<_> = std::fs::read_dir(dir.join("sub"))
            .expect("list target dir after overwrite")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(entries, vec!["a.txt".to_owned()]);
        assert!(fs.read(&dir).is_err());
        let bare = PathBuf::from("dx-apply-bare-tmp.txt");
        fs.write_atomic(&bare, b"bare\n").expect("bare write");
        assert_eq!(fs.read(&bare).expect("bare read"), Some(b"bare\n".to_vec()));
        std::fs::remove_file(&bare).expect("bare cleanup");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn read_failure_maps_to_io_error() {
        let fs = FlakyFs {
            fail_read: true,
            fail_write: false,
        };
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![update("new\n")],
        };
        assert_eq!(
            apply_envelope(&envelope, &root(), &fs, &NoHooks).expect_err("read must fail"),
            ApplyError::Io {
                path: "a.txt".to_owned(),
                message: "denied".to_owned(),
            }
        );
    }

    #[test]
    fn write_failure_maps_to_io_error() {
        let fs = FlakyFs {
            fail_read: false,
            fail_write: true,
        };
        assert_eq!(
            apply_envelope(&create_envelope(), &root(), &fs, &NoHooks)
                .expect_err("write must fail"),
            ApplyError::Io {
                path: "new.txt".to_owned(),
                message: "read-only".to_owned(),
            }
        );
    }

    #[test]
    fn flaky_success_applies_create() {
        let fs = FlakyFs {
            fail_read: false,
            fail_write: false,
        };
        let report = apply_envelope(&create_envelope(), &root(), &fs, &NoHooks).expect("apply");
        assert_eq!(report.applied.len(), 1);
        assert_eq!(report.applied[0].path, "new.txt");
    }
}
