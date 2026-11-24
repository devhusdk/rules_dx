use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathIoError {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("cannot write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("cannot list {}: {source}", path.display())]
    List { path: PathBuf, source: io::Error },
    #[error("cannot create {}: {source}", path.display())]
    Create { path: PathBuf, source: io::Error },
}

impl PathIoError {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Read { path, .. }
            | Self::Write { path, .. }
            | Self::List { path, .. }
            | Self::Create { path, .. } => path,
        }
    }
}

pub fn read(path: &Path) -> Result<Vec<u8>, PathIoError> {
    std::fs::read(path).map_err(|source| PathIoError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_to_string(path: &Path) -> Result<String, PathIoError> {
    std::fs::read_to_string(path).map_err(|source| PathIoError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_scratch(path: &Path, content: &[u8]) -> Result<(), PathIoError> {
    crate::write_scratch(path, content).map_err(|source| PathIoError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub fn create_dir_all(path: &Path) -> Result<(), PathIoError> {
    std::fs::create_dir_all(path).map_err(|source| PathIoError::Create {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_dir(path: &Path) -> Result<Vec<PathBuf>, PathIoError> {
    let list = |source| PathIoError::List {
        path: path.to_path_buf(),
        source,
    };
    let mut paths = std::fs::read_dir(path)
        .map_err(list)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(list))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_keep_the_path_and_the_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt");
        let error = read_to_string(&missing).expect_err("missing file");
        assert_eq!(error.path(), missing);
        assert!(
            matches!(&error, PathIoError::Read { source, .. } if source.kind() == io::ErrorKind::NotFound)
        );
        assert!(error.to_string().starts_with("cannot read "), "{error}");
        assert!(error.to_string().contains("missing.txt"), "{error}");
        let error = read_dir(&missing).expect_err("missing dir");
        assert!(error.to_string().starts_with("cannot list "), "{error}");
        let file = dir.path().join("file");
        write_scratch(&file, b"x").expect("write file");
        let error = create_dir_all(&file.join("sub")).expect_err("dir under a file");
        assert_eq!(error.path(), file.join("sub"));
        assert!(error.to_string().starts_with("cannot create "), "{error}");
    }

    #[test]
    fn write_then_read_round_trips_and_lists_sorted() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_scratch(&dir.path().join("b"), b"two").expect("write b");
        write_scratch(&dir.path().join("a"), b"one").expect("write a");
        assert_eq!(read(&dir.path().join("b")).expect("read"), b"two");
        let names: Vec<_> = read_dir(dir.path())
            .expect("list")
            .iter()
            .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
            .collect();
        assert_eq!(names, ["a", "b"]);
    }
}
