//! Reads workspace files from the Bazel runfiles tree.

use crate::{Source, SupportMatrixError};

/// Reads `_main/<path>`, or `<repo>/<path>` for `@repo/<path>`, through the runfiles library.
pub struct RunfilesSource {
    runfiles: runfiles::Runfiles,
}

impl RunfilesSource {
    /// Opens the runfiles of the running binary or test.
    ///
    /// # Errors
    ///
    /// Returns [`SupportMatrixError::Runfiles`] when the runfiles cannot be opened.
    pub fn create() -> Result<Self, SupportMatrixError> {
        runfiles::Runfiles::create()
            .map(|runfiles| Self { runfiles })
            .map_err(SupportMatrixError::Runfiles)
    }
}

impl Source for RunfilesSource {
    fn read(&self, path: &str) -> Result<String, SupportMatrixError> {
        let key = match path.strip_prefix('@') {
            Some(external) => external.replacen('/', "+/", 1),
            None => format!("_main/{path}"),
        };
        let Some(found) = self.runfiles.rlocation(&key) else {
            return Err(SupportMatrixError::NotInRunfiles {
                path: path.to_owned(),
            });
        };
        dx_atomic_fs::path_io::read_to_string(&found).map_err(|source| SupportMatrixError::Read {
            path: path.to_owned(),
            source,
        })
    }
}
