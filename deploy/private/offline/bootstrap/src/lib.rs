//! Installs Bazel and vendored advisory snapshots from an offline bundle.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use dx_atomic_fs::path_io::{self, PathIoError};

/// The advisory sets an offline bundle may carry.
pub const ADVISORY_SETS: [&str; 6] = ["cargo", "npm", "maven", "nuget", "go", "rubygems"];

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> String {
    "usage: bootstrap-offline --bundle DIR [--install-dir DIR] [--workspace DIR]".to_string()
}

/// Why an invocation or a bootstrap was refused.
#[derive(Debug)]
pub enum BootstrapError {
    FlagNeedsDir(String),
    UnknownArgument(String),
    NoInstallDir,
    MissingBundle,
    BundleNotFound(String),
    BundleUri(dx_path::UriError),
    BundleMissingDir {
        dir: &'static str,
        bundle: String,
    },
    UnsupportedHost {
        os: String,
        arch: String,
    },
    NoHostLauncher(&'static str),
    UnpinnedLauncher(&'static str),
    MissingManifest(PathBuf),
    ManifestMissingFile {
        name: String,
        manifest: PathBuf,
    },
    ManifestEmpty(PathBuf),
    ChecksumMismatch {
        name: String,
        got: String,
        want: String,
    },
    InstalledMismatch {
        target: PathBuf,
        got: String,
        want: String,
    },
    NoSnapshots,
    UnknownSet(String),
    UnpinnedSnapshot {
        named: String,
        manifest: PathBuf,
    },
    NoRecordedDate {
        named: String,
        identity: String,
    },
    UnpinnedIdentity {
        identity: String,
        manifest: PathBuf,
    },
    InvalidRetrievedAt(String),
    RetrievedAfterToday {
        identity: String,
        retrieved: String,
        today: String,
    },
    OtherBytes {
        identity: String,
        bound: String,
        named: String,
        sha: String,
    },
    NoSha256(String),
    OrphanIdentity {
        identity: String,
        stem: String,
    },
    Create {
        path: PathBuf,
        source: io::Error,
    },
    Remove {
        path: PathBuf,
        source: io::Error,
    },
    Permissions {
        path: PathBuf,
        source: io::Error,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
    Write {
        path: PathBuf,
        source: io::Error,
    },
    Io(PathIoError),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FlagNeedsDir(flag) => write!(f, "{flag} needs a DIR"),
            Self::UnknownArgument(flag) => write!(f, "unknown argument '{flag}'"),
            Self::NoInstallDir => {
                f.write_str("neither HOME nor USERPROFILE is set; pass --install-dir DIR")
            }
            Self::MissingBundle => f.write_str("missing --bundle DIR (vendored offline bundle)"),
            Self::BundleNotFound(raw) => write!(f, "bundle dir does not exist: {raw}"),
            Self::BundleUri(error) => write!(f, "bundle path has no file URI: {error}"),
            Self::BundleMissingDir { dir, bundle } => {
                write!(f, "bundle has no {dir}/ dir: {bundle}")
            }
            Self::UnsupportedHost { os, arch } => write!(f, "unsupported host {os}-{arch}"),
            Self::NoHostLauncher(asset) => {
                write!(f, "bundle has no launcher for this host: {asset}")
            }
            Self::UnpinnedLauncher(asset) => {
                write!(f, "manifest does not pin this host launcher: {asset}")
            }
            Self::MissingManifest(path) => write!(f, "missing manifest: {}", path.display()),
            Self::ManifestMissingFile { name, manifest } => write!(
                f,
                "manifest names missing file: {name} (in {})",
                manifest.display()
            ),
            Self::ManifestEmpty(path) => {
                write!(f, "manifest names no files: {}", path.display())
            }
            Self::ChecksumMismatch { name, got, want } => {
                write!(f, "checksum mismatch for {name}: got {got} want {want}")
            }
            Self::InstalledMismatch { target, got, want } => write!(
                f,
                "installed {} does not match the verified launcher: got {got} want {want}",
                target.display()
            ),
            Self::NoSnapshots => f.write_str("bundle carries no advisory snapshots"),
            Self::UnknownSet(set) => write!(f, "bundle carries unknown advisory set: {set}"),
            Self::UnpinnedSnapshot { named, manifest } => write!(
                f,
                "manifest does not pin advisory snapshot: {named} (in {})",
                manifest.display()
            ),
            Self::NoRecordedDate { named, identity } => write!(
                f,
                "bundle has no recorded date for advisory snapshot: {named} \
                 (want advisory/{identity})"
            ),
            Self::UnpinnedIdentity { identity, manifest } => write!(
                f,
                "manifest does not pin advisory identity: {identity} (in {})",
                manifest.display()
            ),
            Self::InvalidRetrievedAt(identity) => write!(
                f,
                "advisory identity {identity} has no valid retrieved_at; want YYYY-MM-DD"
            ),
            Self::RetrievedAfterToday {
                identity,
                retrieved,
                today,
            } => write!(
                f,
                "advisory identity {identity} retrieved_at {retrieved} is after today {today}; \
                 check this host's clock or rebuild the bundle"
            ),
            Self::OtherBytes {
                identity,
                bound,
                named,
                sha,
            } => write!(
                f,
                "advisory identity {identity} is for other bytes: \
                 sha256 {bound} but {named} is {sha}"
            ),
            Self::NoSha256(identity) => write!(f, "advisory identity {identity} has no sha256"),
            Self::OrphanIdentity { identity, stem } => write!(
                f,
                "advisory identity {identity} has no snapshot; want advisory/{stem}.json"
            ),
            Self::Create { path, source } => {
                write!(f, "cannot create {}: {source}", path.display())
            }
            Self::Remove { path, source } => {
                write!(f, "cannot remove {}: {source}", path.display())
            }
            Self::Permissions { path, source } => {
                write!(f, "cannot make {} executable: {source}", path.display())
            }
            Self::Copy { from, to, source } => write!(
                f,
                "cannot copy {} to {}: {source}",
                from.display(),
                to.display()
            ),
            Self::Write { path, source } => {
                write!(f, "cannot write {}: {source}", path.display())
            }
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BootstrapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BundleUri(error) => Some(error),
            Self::Create { source, .. }
            | Self::Remove { source, .. }
            | Self::Permissions { source, .. }
            | Self::Copy { source, .. }
            | Self::Write { source, .. } => Some(source),
            Self::Io(error) => error.source(),
            Self::FlagNeedsDir(_)
            | Self::UnknownArgument(_)
            | Self::NoInstallDir
            | Self::MissingBundle
            | Self::BundleNotFound(_)
            | Self::BundleMissingDir { .. }
            | Self::UnsupportedHost { .. }
            | Self::NoHostLauncher(_)
            | Self::UnpinnedLauncher(_)
            | Self::MissingManifest(_)
            | Self::ManifestMissingFile { .. }
            | Self::ManifestEmpty(_)
            | Self::ChecksumMismatch { .. }
            | Self::InstalledMismatch { .. }
            | Self::NoSnapshots
            | Self::UnknownSet(_)
            | Self::UnpinnedSnapshot { .. }
            | Self::NoRecordedDate { .. }
            | Self::UnpinnedIdentity { .. }
            | Self::InvalidRetrievedAt(_)
            | Self::RetrievedAfterToday { .. }
            | Self::OtherBytes { .. }
            | Self::NoSha256(_)
            | Self::OrphanIdentity { .. } => None,
        }
    }
}

impl From<PathIoError> for BootstrapError {
    fn from(error: PathIoError) -> Self {
        Self::Io(error)
    }
}

/// The parsed invocation, with defaults already applied.
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub bundle: Option<String>,
    pub install_dir: String,
    pub workspace: String,
}

/// What one invocation asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Print the usage line and succeed.
    Help,
    /// Run the bootstrap with these options.
    Bootstrap(Options),
}

/// Parses argv, or returns the refusal of a bad argument.
///
/// # Errors
///
/// Returns the refusal when an argument is not recognised.
pub fn parse_args(args: &[String]) -> Result<Command, BootstrapError> {
    let mut options = Options {
        bundle: None,
        install_dir: String::new(),
        workspace: std::env::current_dir()
            .map_or_else(|_| ".".to_string(), |p| p.to_string_lossy().into_owned()),
    };
    let mut install_dir = None;
    let mut index = 0;
    while let Some(flag) = args.get(index).map(String::as_str) {
        if matches!(flag, "--bundle" | "--install-dir" | "--workspace") && args.len() < index + 2 {
            return Err(BootstrapError::FlagNeedsDir(flag.to_string()));
        }
        match flag {
            "--bundle" => options.bundle = args.get(index + 1).cloned(),
            "--install-dir" => install_dir = args.get(index + 1).cloned(),
            "--workspace" => {
                if let Some(value) = args.get(index + 1) {
                    options.workspace.clone_from(value);
                }
            }
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(BootstrapError::UnknownArgument(flag.to_string())),
        }
        index += 2;
    }
    let home = std::env::var("HOME").ok();
    let profile = std::env::var("USERPROFILE").ok();
    options.install_dir = install_dir
        .or_else(|| default_install_dir(home.as_deref(), profile.as_deref()))
        .ok_or(BootstrapError::NoInstallDir)?;
    Ok(Command::Bootstrap(options))
}

/// Returns `bin` under `HOME`, else under `USERPROFILE` on native Windows.
#[must_use]
pub fn default_install_dir(home: Option<&str>, profile: Option<&str>) -> Option<String> {
    let base = home
        .filter(|h| !h.is_empty())
        .or(profile.filter(|p| !p.is_empty()))?;
    Some(
        PathBuf::from(base)
            .join(".local")
            .join("bin")
            .to_string_lossy()
            .into_owned(),
    )
}

/// Returns the launcher file name this host needs from the bundle.
#[must_use]
pub fn host_launcher(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("bazelisk-linux-amd64"),
        ("linux", "aarch64") => Some("bazelisk-linux-arm64"),
        ("macos", "x86_64") => Some("bazelisk-darwin-amd64"),
        ("macos", "aarch64") => Some("bazelisk-darwin-arm64"),
        ("windows", "x86_64") => Some("bazelisk-windows-amd64.exe"),
        _ => None,
    }
}

/// Returns today's date as `YYYY-MM-DD` in UTC.
#[must_use]
pub fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs() / 86_400).unwrap_or(0));
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * shifted_month + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    })
    .unwrap_or(1);
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Returns whether one value is a real calendar day written `YYYY-MM-DD`.
#[must_use]
pub fn is_plain_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !value.is_ascii()
        || bytes.len() != 10
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
    {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = value.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=last).contains(&day)
}

/// Reads the first quoted string value of `key` from one identity file.
///
/// This is not a JSON reader: it takes `"key"`, optional whitespace, a colon,
/// optional whitespace and a quoted value with no backslash. It matches the
/// pretty and compact forms `advisory_prep` writes. The first match wins.
#[must_use]
pub fn recorded_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut from = 0usize;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find(&needle)) {
        let start = from + found;
        let after = text.get(start + needle.len()..)?;
        let before = text.get(..start)?.trim_end();
        // A key opens an object or follows a comma; a value never does.
        if (before.ends_with('{') || before.ends_with(','))
            && let Some(rest) = after.trim_start().strip_prefix(':')
        {
            let rest = rest.trim_start();
            let body = rest.strip_prefix('"')?;
            let (value, _) = body.split_once('"')?;
            return (!value.contains('\\')).then(|| value.to_string());
        }
        from = start + needle.len();
    }
    None
}

/// One `SHA256SUMS` line: the wanted digest and the file it names.
pub fn manifest_entry(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut parts = line.splitn(2, char::is_whitespace);
    let want = parts.next()?.trim().to_string();
    let name = parts.next().unwrap_or("").trim().to_string();
    Some((want, name))
}

/// Returns the digest the manifest pins for one file name.
#[must_use]
pub fn pinned(manifest: &str, name: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (want, named) = manifest_entry(line)?;
        (named == name).then_some(want)
    })
}

/// Installs the host launcher and populates the advisory snapshots.
///
/// # Errors
///
/// Returns the refusal when the bundle, its manifests or the install fail.
pub fn run(options: &Options, os: &str, arch: &str, today: &str) -> Result<(), BootstrapError> {
    let Some(raw) = options.bundle.as_deref().filter(|b| !b.is_empty()) else {
        return Err(BootstrapError::MissingBundle);
    };
    let bundle = match std::fs::canonicalize(raw) {
        Ok(path) if path.is_dir() => path,
        _ => return Err(BootstrapError::BundleNotFound(raw.to_string())),
    };
    let url = dx_path::uri(&bundle).map_err(BootstrapError::BundleUri)?;
    let root = bundle.clone();
    let bazelisk = root.join("bazelisk");
    let missing_dir = |dir| BootstrapError::BundleMissingDir {
        dir,
        bundle: raw.to_string(),
    };
    if !bazelisk.is_dir() {
        return Err(missing_dir("bazelisk"));
    }
    let advisory = root.join("advisory");
    if !advisory.is_dir() {
        return Err(missing_dir("advisory"));
    }
    let Some(asset) = host_launcher(os, arch) else {
        return Err(BootstrapError::UnsupportedHost {
            os: os.to_string(),
            arch: arch.to_string(),
        });
    };
    let source = bazelisk.join(asset);
    if !source.is_file() {
        return Err(BootstrapError::NoHostLauncher(asset));
    }

    let bazelisk_manifest = verify(&bazelisk)?;
    let advisory_manifest = verify(&advisory)?;
    let Some(want) = pinned(&bazelisk_manifest, asset) else {
        return Err(BootstrapError::UnpinnedLauncher(asset));
    };
    let got = digest(&source);
    if got != want {
        return Err(BootstrapError::ChecksumMismatch {
            name: asset.to_string(),
            got,
            want,
        });
    }

    let installed = install(&source, &options.install_dir, asset, &want)?;

    let mut snapshots: Vec<PathBuf> = std::fs::read_dir(&advisory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter(|path| !is_identity(path))
        .collect();
    snapshots.sort();
    let mut identities: Vec<String> = std::fs::read_dir(&advisory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".meta.json"))
        .collect();
    identities.sort();
    let populated =
        dx_path::state::local_dir(Path::new(&options.workspace)).join(dx_path::state::ADVISORY);
    std::fs::create_dir_all(&populated).map_err(|source| BootstrapError::Create {
        path: populated.clone(),
        source,
    })?;
    let mut count = 0usize;
    for snapshot in &snapshots {
        if bind_snapshot(
            snapshot,
            &advisory,
            &advisory_manifest,
            &url,
            &populated,
            today,
        )? {
            count += 1;
        }
    }
    orphan_identity(&identities, &snapshots)?;
    if count == 0 {
        return Err(BootstrapError::NoSnapshots);
    }

    report(&installed, &got, count, &populated);
    Ok(())
}

fn report(installed: &Path, got: &str, count: usize, populated: &Path) {
    println!(
        "bootstrap-offline: installed {} (sha256 {got}, no network)",
        installed.display()
    );
    println!(
        "bootstrap-offline: populated {count} advisory snapshots under {}",
        populated.display()
    );
    println!(
        "bootstrap-offline: first Bazel module/toolchain fetch still needs network once; \
         steady-state offline after"
    );
}

fn bind_snapshot(
    snapshot: &Path,
    advisory: &Path,
    advisory_manifest: &str,
    url: &str,
    populated: &Path,
    today: &str,
) -> Result<bool, BootstrapError> {
    let Some(set) = snapshot
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
    else {
        return Ok(false);
    };
    if !ADVISORY_SETS.contains(&set.as_str()) {
        return Err(BootstrapError::UnknownSet(set));
    }
    let named = format!("{set}.json");
    if pinned(advisory_manifest, &named).is_none() {
        return Err(BootstrapError::UnpinnedSnapshot {
            named,
            manifest: advisory.join("SHA256SUMS"),
        });
    }
    let sha = digest(snapshot);
    let identity = format!("{set}.meta.json");
    let identity_path = advisory.join(&identity);
    if !identity_path.is_file() {
        return Err(BootstrapError::NoRecordedDate { named, identity });
    }
    if pinned(advisory_manifest, &identity).is_none() {
        return Err(BootstrapError::UnpinnedIdentity {
            identity,
            manifest: advisory.join("SHA256SUMS"),
        });
    }
    let recorded = path_io::read_to_string(&identity_path)?;
    let retrieved = match recorded_field(&recorded, "retrieved_at") {
        Some(date) if is_plain_date(&date) => date,
        _ => return Err(BootstrapError::InvalidRetrievedAt(identity)),
    };
    if retrieved.as_str() > today {
        return Err(BootstrapError::RetrievedAfterToday {
            identity,
            retrieved,
            today: today.to_string(),
        });
    }
    match recorded_field(&recorded, "sha256") {
        Some(bound) if bound == sha => {}
        Some(bound) => {
            return Err(BootstrapError::OtherBytes {
                identity,
                bound,
                named,
                sha,
            });
        }
        None => return Err(BootstrapError::NoSha256(identity)),
    }
    let meta = format!(
        "{{\"set\": \"{set}\", \"url\": \"{url}/advisory/{named}\", \
         \"sha256\": \"{sha}\", \"retrieved_at\": \"{retrieved}\", \
         \"path\": \".dx/local/advisory/{named}\"}}"
    );
    publish_set(snapshot, populated, &set, &meta, write)?;
    Ok(true)
}

fn orphan_identity(identities: &[String], snapshots: &[PathBuf]) -> Result<(), BootstrapError> {
    let orphan = identities.iter().find_map(|identity| {
        let stem = identity.trim_end_matches(".meta.json");
        let has_snapshot = snapshots
            .iter()
            .any(|path| path.file_stem().is_some_and(|s| s == stem));
        (!has_snapshot).then(|| BootstrapError::OrphanIdentity {
            identity: identity.clone(),
            stem: stem.to_string(),
        })
    });
    orphan.map_or(Ok(()), Err)
}

fn is_identity(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(".meta.json"))
}

/// Checks every file `dir/SHA256SUMS` names and returns the manifest text.
fn verify(dir: &Path) -> Result<String, BootstrapError> {
    let path = dir.join("SHA256SUMS");
    if !path.is_file() {
        return Err(BootstrapError::MissingManifest(path));
    }
    let manifest = path_io::read_to_string(&path)?;
    let mut verified = 0usize;
    for line in manifest.lines() {
        let Some((want, name)) = manifest_entry(line) else {
            continue;
        };
        let file = dir.join(&name);
        if name.is_empty() || !file.is_file() {
            let named = if name.is_empty() { "<empty>" } else { &name };
            return Err(BootstrapError::ManifestMissingFile {
                name: named.to_string(),
                manifest: path,
            });
        }
        let got = digest(&file);
        if got != want {
            return Err(BootstrapError::ChecksumMismatch { name, got, want });
        }
        verified += 1;
    }
    if verified == 0 {
        return Err(BootstrapError::ManifestEmpty(path));
    }
    Ok(manifest)
}

fn install(
    source: &Path,
    install_dir: &str,
    asset: &str,
    want: &str,
) -> Result<PathBuf, BootstrapError> {
    let dir = Path::new(install_dir);
    std::fs::create_dir_all(dir).map_err(|source| BootstrapError::Create {
        path: dir.to_path_buf(),
        source,
    })?;
    let name = if Path::new(asset)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        "bazel.exe"
    } else {
        "bazel"
    };
    let target = dir.join(name);
    copy(source, &target)?;
    let got = digest(&target);
    if got != want {
        return Err(BootstrapError::InstalledMismatch {
            target,
            got,
            want: want.to_string(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).map_err(
            |source| BootstrapError::Permissions {
                path: target.clone(),
                source,
            },
        )?;
    }
    Ok(target)
}

fn copy(from: &Path, to: &Path) -> Result<(), BootstrapError> {
    dx_atomic_fs::remove_stale_staging(to);
    dx_atomic_fs::copy_atomic(from, to).map_err(|source| BootstrapError::Copy {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

/// Replaces one populated set. The old identity goes first and the new one is
/// written last, so an interrupt leaves a missing identity, never a wrong one.
fn publish_set(
    snapshot: &Path,
    populated: &Path,
    set: &str,
    meta: &str,
    write_identity: fn(&Path, &str) -> Result<(), BootstrapError>,
) -> Result<(), BootstrapError> {
    let identity = populated.join(format!("{set}.meta.json"));
    match std::fs::remove_file(&identity) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(BootstrapError::Remove {
                path: identity,
                source,
            });
        }
    }
    copy(snapshot, &populated.join(format!("{set}.json")))?;
    write_identity(&identity, meta)
}

fn write(target: &Path, body: &str) -> Result<(), BootstrapError> {
    dx_atomic_fs::write_atomic(target, body.as_bytes()).map_err(|source| BootstrapError::Write {
        path: target.to_path_buf(),
        source,
    })
}

fn digest(path: &Path) -> String {
    dx_digest::sha256_file_hex(path).unwrap_or_default()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
