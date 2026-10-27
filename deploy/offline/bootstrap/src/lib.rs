//! Installs Bazel and vendored advisory snapshots from an offline bundle.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The advisory sets an offline bundle may carry.
pub const ADVISORY_SETS: [&str; 6] = ["cargo", "npm", "maven", "nuget", "go", "rubygems"];

/// The usage line shown for a rejected invocation.
pub fn usage() -> String {
    "usage: bootstrap-offline --bundle DIR [--install-dir DIR] [--workspace DIR]".to_string()
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

/// Parses argv, or returns the plain refusal text of a bad argument.
pub fn parse_args(args: &[String]) -> Result<Command, String> {
    let home = std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    let mut options = Options {
        bundle: None,
        install_dir: format!("{home}/.local/bin"),
        workspace: std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".to_string()),
    };
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--bundle" | "--install-dir" | "--workspace") && args.len() < index + 2 {
            return Err(format!("{flag} needs a DIR"));
        }
        match flag {
            "--bundle" => options.bundle = args.get(index + 1).cloned(),
            "--install-dir" => options.install_dir = args[index + 1].clone(),
            "--workspace" => options.workspace = args[index + 1].clone(),
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(format!("unknown argument '{flag}'")),
        }
        index += 2;
    }
    Ok(Command::Bootstrap(options))
}

/// Returns the launcher file name this host needs from the bundle.
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
pub fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
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
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Returns whether one value is a real calendar day written `YYYY-MM-DD`.
pub fn is_plain_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !value.is_ascii() || bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &value[range];
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
pub fn recorded_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut from = 0usize;
    while let Some(found) = text[from..].find(&needle) {
        let start = from + found;
        let after = &text[start + needle.len()..];
        let before = text[..start].trim_end();
        // A key opens an object or follows a comma; a value never does.
        if before.ends_with('{') || before.ends_with(',') {
            if let Some(rest) = after.trim_start().strip_prefix(':') {
                let rest = rest.trim_start();
                let body = rest.strip_prefix('"')?;
                let value = &body[..body.find('"')?];
                return (!value.contains('\\')).then(|| value.to_string());
            }
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
pub fn pinned(manifest: &str, name: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (want, named) = manifest_entry(line)?;
        (named == name).then_some(want)
    })
}

/// Returns the refusal text, or an empty string when the bootstrap succeeded.
pub fn run(options: &Options, os: &str, arch: &str, today: &str) -> String {
    let Some(raw) = options.bundle.as_deref().filter(|b| !b.is_empty()) else {
        return "missing --bundle DIR (vendored offline bundle)".to_string();
    };
    let bundle = match std::fs::canonicalize(raw) {
        Ok(path) if path.is_dir() => path,
        _ => return format!("bundle dir does not exist: {raw}"),
    };
    let url = match dx_path::uri(&bundle) {
        Ok(url) => url,
        Err(error) => return format!("bundle path has no file URI: {error}"), // LCOV_EXCL_LINE - reason: a canonical absolute path always has a file URI, issue: 2550, policy: docs/cli/commands/build-test-coverage.md
    };
    let root = bundle.clone();
    let bazelisk = root.join("bazelisk");
    if !bazelisk.is_dir() {
        return format!("bundle has no bazelisk/ dir: {raw}");
    }
    let advisory = root.join("advisory");
    if !advisory.is_dir() {
        return format!("bundle has no advisory/ dir: {raw}");
    }
    let Some(asset) = host_launcher(os, arch) else {
        return format!("unsupported host {os}-{arch}");
    };
    let source = bazelisk.join(asset);
    if !source.is_file() {
        return format!("bundle has no launcher for this host: {asset}");
    }

    let bazelisk_manifest = read(&bazelisk.join("SHA256SUMS"));
    if let Some(refusal) = verify(&bazelisk, &bazelisk_manifest) {
        return refusal;
    }
    let advisory_manifest = read(&advisory.join("SHA256SUMS"));
    if let Some(refusal) = verify(&advisory, &advisory_manifest) {
        return refusal;
    }
    let Some(want) = pinned(&bazelisk_manifest, asset) else {
        return format!("manifest does not pin this host launcher: {asset}");
    };
    let got = digest(&source);
    if got != want {
        return format!("checksum mismatch for {asset}: got {got} want {want}");
    }

    let installed = match install(&source, &options.install_dir, asset, &want) {
        Ok(path) => path,
        Err(detail) => return detail,
    };

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
    let populated = PathBuf::from(&options.workspace)
        .join(".dx")
        .join("advisory");
    if let Err(error) = std::fs::create_dir_all(&populated) {
        return format!("cannot create {}: {error}", populated.display());
    }
    let mut count = 0usize;
    for snapshot in &snapshots {
        let Some(set) = snapshot
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
        else {
            continue;
        };
        if !ADVISORY_SETS.contains(&set.as_str()) {
            return format!("bundle carries unknown advisory set: {set}");
        }
        let named = format!("{set}.json");
        if pinned(&advisory_manifest, &named).is_none() {
            return format!(
                "manifest does not pin advisory snapshot: {named} (in {})",
                advisory.join("SHA256SUMS").display()
            );
        }
        let sha = digest(snapshot);
        let identity = format!("{set}.meta.json");
        let identity_path = advisory.join(&identity);
        if !identity_path.is_file() {
            return format!(
                "bundle has no recorded date for advisory snapshot: {named} \
                 (want advisory/{identity})"
            );
        }
        if pinned(&advisory_manifest, &identity).is_none() {
            return format!(
                "manifest does not pin advisory identity: {identity} (in {})",
                advisory.join("SHA256SUMS").display()
            );
        }
        let recorded = read(&identity_path);
        let retrieved = match recorded_field(&recorded, "retrieved_at") {
            Some(date) if is_plain_date(&date) => date,
            _ => {
                return format!(
                    "advisory identity {identity} has no valid retrieved_at; want YYYY-MM-DD"
                )
            }
        };
        if retrieved.as_str() > today {
            return format!(
                "advisory identity {identity} retrieved_at {retrieved} is after today {today}; \
                 check this host's clock or rebuild the bundle"
            );
        }
        match recorded_field(&recorded, "sha256") {
            Some(bound) if bound == sha => {}
            Some(bound) => {
                return format!(
                    "advisory identity {identity} is for other bytes: \
                     sha256 {bound} but {named} is {sha}"
                )
            }
            None => return format!("advisory identity {identity} has no sha256"),
        }
        let target = populated.join(&named);
        if let Err(detail) = copy(snapshot, &target) {
            return detail;
        }
        let meta = format!(
            "{{\"set\": \"{set}\", \"url\": \"{url}/advisory/{named}\", \
             \"sha256\": \"{sha}\", \"retrieved_at\": \"{retrieved}\", \
             \"path\": \".dx/advisory/{named}\"}}"
        );
        if let Err(detail) = write(&populated.join(format!("{set}.meta.json")), &meta) {
            return detail;
        }
        count += 1;
    }
    for identity in &identities {
        let stem = identity.trim_end_matches(".meta.json");
        if !snapshots
            .iter()
            .any(|path| path.file_stem().is_some_and(|s| s == stem))
        {
            return format!(
                "advisory identity {identity} has no snapshot; want advisory/{stem}.json"
            );
        }
    }
    if count == 0 {
        return "bundle carries no advisory snapshots".to_string();
    }

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
    String::new()
}

fn is_identity(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(".meta.json"))
}

fn verify(dir: &Path, manifest: &str) -> Option<String> {
    let path = dir.join("SHA256SUMS");
    if !path.is_file() {
        return Some(format!("missing manifest: {}", path.display()));
    }
    let mut verified = 0usize;
    for line in manifest.lines() {
        let Some((want, name)) = manifest_entry(line) else {
            continue;
        };
        let file = dir.join(&name);
        if name.is_empty() || !file.is_file() {
            let named = if name.is_empty() { "<empty>" } else { &name };
            return Some(format!(
                "manifest names missing file: {named} (in {})",
                path.display()
            ));
        }
        let got = digest(&file);
        if got != want {
            return Some(format!(
                "checksum mismatch for {name}: got {got} want {want}"
            ));
        }
        verified += 1;
    }
    if verified == 0 {
        return Some(format!("manifest names no files: {}", path.display()));
    }
    None
}

fn install(source: &Path, install_dir: &str, asset: &str, want: &str) -> Result<PathBuf, String> {
    let dir = Path::new(install_dir);
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("cannot create {install_dir}: {error}"))?;
    let name = if asset.ends_with(".exe") {
        "bazel.exe"
    } else {
        "bazel"
    };
    let target = dir.join(name);
    copy(source, &target)?;
    let got = digest(&target);
    if got != want {
        return Err(format!(
            "installed {} does not match the verified launcher: got {got} want {want}",
            target.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("cannot make {} executable: {error}", target.display()))?;
    }
    Ok(target)
}

fn copy(source: &Path, target: &Path) -> Result<(), String> {
    dx_atomic_fs::copy_atomic(source, target).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })
}

fn write(target: &Path, body: &str) -> Result<(), String> {
    std::fs::write(target, body)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn digest(path: &Path) -> String {
    dx_digest::sha256_file_hex(path).unwrap_or_default()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
