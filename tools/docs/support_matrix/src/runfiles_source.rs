//! Reads workspace files from the Bazel runfiles tree.

use crate::Source;

/// Reads `_main/<path>` through the runfiles library.
pub struct RunfilesSource {
    runfiles: runfiles::Runfiles,
}

impl RunfilesSource {
    /// Opens the runfiles of the running binary or test.
    pub fn create() -> Result<Self, String> {
        runfiles::Runfiles::create()
            .map(|runfiles| Self { runfiles })
            .map_err(|error| format!("cannot open runfiles: {error}"))
    }
}

impl Source for RunfilesSource {
    fn read(&self, path: &str) -> Result<String, String> {
        let key = format!("_main/{path}");
        let Some(found) = self.runfiles.rlocation(&key) else {
            return Err(format!("{path} is not in the runfiles"));
        };
        std::fs::read_to_string(&found).map_err(|error| {
            format!("read {path}: {error}; add it to the support_matrix data and export it")
        })
    }
}
