//! Downloads the trivy vulnerability and checks databases next to the OSV snapshots.

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use dx_atomic_fs::write_atomic;
use dx_audit::advisory_prep::PrepError;
use serde::{Deserialize, Serialize};

pub const CACHE_DIR: &str = "trivy";
pub const META_FILE: &str = "trivy.meta.json";

/// What one trivy preparation run left in the cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrivyPrepared {
    pub cache: PathBuf,
    pub retrieved_at: String,
    pub db_updated_at: String,
    pub fetched: bool,
}

#[derive(Deserialize, Serialize)]
struct Sidecar {
    retrieved_at: String,
    db_updated_at: String,
}

#[derive(Deserialize)]
struct DbMetadata {
    #[serde(rename = "UpdatedAt")]
    updated_at: String,
}

/// The arguments of the two download steps, vulnerability database then checks.
#[must_use]
pub fn download_args(cache: &Path) -> [Vec<String>; 2] {
    let cache = cache.display().to_string();
    let base = |flag: &str| {
        vec![
            "--cache-dir".to_owned(),
            cache.clone(),
            "image".to_owned(),
            flag.to_owned(),
        ]
    };
    [base("--download-db-only"), base("--download-checks-only")]
}

/// Runs the pinned binary for both downloads unless the sidecar already carries `retrieved_at`.
pub fn prepare_trivy(
    binary: &Path,
    out: &Path,
    retrieved_at: &str,
) -> Result<TrivyPrepared, PrepError> {
    prepare_with(out, retrieved_at, |args| run(binary, args))
}

pub fn prepare_with(
    out: &Path,
    retrieved_at: &str,
    run: impl Fn(&[String]) -> Result<(), PrepError>,
) -> Result<TrivyPrepared, PrepError> {
    let cache = out.join(CACHE_DIR);
    let meta_path = out.join(META_FILE);
    let metadata_path = cache.join("db").join("metadata.json");
    if metadata_path.is_file()
        && let Ok(text) = std::fs::read_to_string(&meta_path)
        && let Ok(sidecar) = serde_json::from_str::<Sidecar>(&text)
        && sidecar.retrieved_at == retrieved_at
    {
        return Ok(TrivyPrepared {
            cache,
            retrieved_at: sidecar.retrieved_at,
            db_updated_at: sidecar.db_updated_at,
            fetched: false,
        });
    }
    for args in download_args(&cache) {
        run(&args)?;
    }
    let metadata = std::fs::read_to_string(&metadata_path).map_err(|error| PrepError::Fetch {
        program: "trivy".to_owned(),
        detail: format!("{}: {error}", metadata_path.display()),
    })?;
    let db: DbMetadata = serde_json::from_str(&metadata).map_err(|error| PrepError::Fetch {
        program: "trivy".to_owned(),
        detail: format!("{}: {error}", metadata_path.display()),
    })?;
    let sidecar = Sidecar {
        retrieved_at: retrieved_at.to_owned(),
        db_updated_at: db.updated_at,
    };
    let write_error = |detail: String| PrepError::Write {
        path: meta_path.display().to_string(),
        detail,
    };
    let mut meta =
        serde_json::to_string(&sidecar).map_err(|error| write_error(error.to_string()))?;
    meta.push('\n');
    write_atomic(&meta_path, meta.as_bytes()).map_err(|error| write_error(error.to_string()))?;
    Ok(TrivyPrepared {
        cache,
        retrieved_at: sidecar.retrieved_at,
        db_updated_at: sidecar.db_updated_at,
        fetched: true,
    })
}

/// Packs the trivy cache directory into a reproducible `.tar.gz` at `dest` and returns its bytes.
pub fn pack_cache(cache: &Path, dest: &Path) -> Result<Vec<u8>, PrepError> {
    let fail = |detail: String| PrepError::Write {
        path: dest.display().to_string(),
        detail,
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| fail(error.to_string()))?;
    }
    let mut files = Vec::new();
    collect(cache, Path::new(""), &mut files)
        .map_err(|error| fail(format!("{}: {error}", cache.display())))?;
    files.sort();
    let mut writer = dx_archive::TarWriter::new();
    for relative in &files {
        let source = cache.join(relative);
        let data = std::fs::read(&source)
            .map_err(|error| fail(format!("{}: {error}", source.display())))?;
        let name = relative
            .iter()
            .map(|part| part.to_str())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail(format!("{} is not UTF-8", relative.display())))?
            .join("/");
        writer
            .add_file(&name, &data, 0o644)
            .map_err(|error| fail(error.to_string()))?;
    }
    let bytes = writer.finish().map_err(|error| fail(error.to_string()))?;
    dx_atomic_fs::write_atomic(dest, &bytes).map_err(|error| fail(error.to_string()))?;
    Ok(bytes)
}

fn collect(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect(root, &path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn run(binary: &Path, args: &[String]) -> Result<(), PrepError> {
    let done = Process::new(binary)
        .args(args)
        .status()
        .map_err(|error| PrepError::Fetch {
            program: binary.display().to_string(),
            detail: error.to_string(),
        })?;
    if done.success() {
        return Ok(());
    }
    Err(PrepError::Fetch {
        program: binary.display().to_string(),
        detail: format!("{} exited with {done}", args.join(" ")),
    })
}

#[cfg(test)]
#[path = "trivy_tests.rs"]
mod trivy_tests;
