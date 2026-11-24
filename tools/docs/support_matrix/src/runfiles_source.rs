use crate::{Source, SupportMatrixError};

pub struct RunfilesSource {
    runfiles: runfiles::Runfiles,
}

impl RunfilesSource {
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
