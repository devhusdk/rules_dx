//! Usage: tiers MANIFEST `STANDARD_OUT` `STRICT_OUT`. Tier paths in the manifest resolve against the working directory.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::process::ExitCode;

use dx_atomic_fs::path_io::{self, PathIoError};
use tiers::{TiersError, parse_manifest, standard_page, strict_page};

#[derive(Debug)]
enum RunError {
    Usage,
    Io(PathIoError),
    Tiers(TiersError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => f.write_str("usage: tiers MANIFEST STANDARD_OUT STRICT_OUT"),
            Self::Io(error) => error.fmt(f),
            Self::Tiers(error) => error.fmt(f),
        }
    }
}

impl From<PathIoError> for RunError {
    fn from(error: PathIoError) -> Self {
        Self::Io(error)
    }
}

impl From<TiersError> for RunError {
    fn from(error: TiersError) -> Self {
        Self::Tiers(error)
    }
}

fn run() -> Result<(), RunError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [manifest, standard_out, strict_out] = &args[..] else {
        return Err(RunError::Usage);
    };
    let text = path_io::read_to_string(Path::new(manifest))?;
    let tiers = parse_manifest(&text)?;
    let mut files = BTreeMap::new();
    for tier in &tiers {
        for path in [&tier.standard_path, &tier.strict_path] {
            let body = path_io::read_to_string(Path::new(path))?;
            files.insert(path.clone(), body);
        }
    }
    let standard = standard_page(&tiers, &files)?;
    let strict = strict_page(&tiers, &files)?;
    path_io::write_scratch(Path::new(standard_out), standard.as_bytes())?;
    path_io::write_scratch(Path::new(strict_out), strict.as_bytes())?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tiers: {error}");
            ExitCode::FAILURE
        }
    }
}
