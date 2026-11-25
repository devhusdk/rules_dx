use crate::api_compat::{
    ApiCheckError, ApiCheckMode, Baseline, Outcome, changes_or_failure, conclude, export_git_dir,
    fetch, fetch_to, latest_published, previous_git_tag, workspace_dir,
};
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    placeholder_refusal, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_ecosystems::Ecosystem;
use dx_process::capture::RunError;
use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum CratesError {
    Paths(io::Error),
    Stage(StageError),
    NoManifest,
    StagedNoManifest,
    NameMismatch {
        found: Option<String>,
        target: String,
    },
    Refused(String),
    Io(PathIoError),
    Run(RunError),
    CargoFailed {
        command: &'static str,
        status: String,
        lines: Vec<String>,
    },
    TempDir(io::Error),
    BaselineExport(ApiCheckError),
    ApiCheck(ApiCheckError),
    NotPublished {
        name: String,
        version: String,
    },
    ChecksumMismatch {
        file_name: String,
        expected: String,
        got: String,
    },
    ForeignMember {
        file_name: String,
        member: String,
        reason: &'static str,
    },
    Fetch(ApiCheckError),
    InvalidCrate {
        file_name: String,
        source: io::Error,
    },
}

impl fmt::Display for CratesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paths(source) | Self::TempDir(source) => write!(f, "crates_deploy: {source}"),
            Self::Stage(source) => write!(f, "crates_deploy: {source}"),
            Self::NoManifest => write!(
                f,
                "crates_deploy: crate has no {}; refusing",
                manifest_name()
            ),
            Self::StagedNoManifest => write!(
                f,
                "crates_deploy: staged crate is missing {}; refusing",
                manifest_name()
            ),
            Self::NameMismatch { found, target } => write!(
                f,
                "crates_deploy: {} is {}, target is {target}; refusing",
                manifest_name(),
                found.as_deref().unwrap_or("(not a plain value)")
            ),
            Self::Refused(message) => f.write_str(message),
            Self::Io(source) => write!(f, "crates_deploy: {source}"),
            Self::Run(source) => write!(f, "crates_deploy: {source}"),
            Self::CargoFailed {
                command,
                status,
                lines,
            } => {
                write!(f, "crates_deploy: {command} failed with {status}")?;
                for line in lines {
                    write!(f, "\ncargo: {line}")?;
                }
                Ok(())
            }
            Self::BaselineExport(source) => write!(f, "crates_deploy: api compat: {source}"),
            Self::ApiCheck(source) => write!(f, "{source}"),
            Self::NotPublished { name, version } => {
                write!(f, "crates_deploy: crates.io does not list {name} {version}")
            }
            Self::ChecksumMismatch {
                file_name,
                expected,
                got,
            } => write!(
                f,
                "crates_deploy: {file_name} has sha256 {got}, the index lists {expected}; refusing"
            ),
            Self::ForeignMember {
                file_name,
                member,
                reason,
            } => write!(
                f,
                "crates_deploy: {file_name} member {member} {reason}; refusing"
            ),
            Self::Fetch(source) => write!(f, "crates_deploy: {source}"),
            Self::InvalidCrate { file_name, source } => {
                write!(
                    f,
                    "crates_deploy: {file_name} is not a valid crate: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CratesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Paths(source) | Self::TempDir(source) => Some(source),
            Self::Stage(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Run(source) => Some(source),
            Self::BaselineExport(source) | Self::ApiCheck(source) | Self::Fetch(source) => {
                Some(source)
            }
            Self::InvalidCrate { source, .. } => Some(source),
            Self::NoManifest
            | Self::StagedNoManifest
            | Self::NameMismatch { .. }
            | Self::Refused(_)
            | Self::CargoFailed { .. }
            | Self::NotPublished { .. }
            | Self::ChecksumMismatch { .. }
            | Self::ForeignMember { .. } => None,
        }
    }
}

fn manifest_name() -> &'static str {
    Ecosystem::Cargo.manifest_name()
}

pub fn crates_package_paths(
    crate_files: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    if crate_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crates vendor: need at least one crate source file",
        ));
    }
    package_relative_paths(crate_files, manifest_name(), "crates vendor")
}

pub fn crates_build_vendor(
    crate_files: &[std::path::PathBuf],
    outdir: &Path,
    crate_name: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    let mapped = crates_package_paths(crate_files)?;
    let house = outdir.join(format!("{crate_name}-vendor"));
    let vendor_dir = house.join("vendor").join(crate_name);
    let registry_dir = house.join("registry").join(crate_name).join(version);
    std::fs::create_dir_all(&vendor_dir)?;
    std::fs::create_dir_all(&registry_dir)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    for (src, base) in &mapped {
        let want = dx_digest::sha256_file_hex(src)?;
        for dest_dir in [&vendor_dir, &registry_dir] {
            let dest = dest_dir.join(base);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            dx_atomic_fs::copy_atomic(src, &dest)?;
            let got = dx_digest::sha256_file_hex(&dest)?;
            if want != got {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "crates vendor: byte mismatch for {base} (expected sha256 {want}, got {got})"
                    ),
                ));
            }
        }
        entries.push((base.clone(), want));
    }
    entries.sort();
    let package_digest = dx_digest::sha256_hex(
        entries
            .iter()
            .map(|(base, digest)| format!("{base}:{digest}"))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    let mut files = serde_json::Map::new();
    for (base, digest) in &entries {
        files.insert(base.clone(), serde_json::Value::String(digest.clone()));
    }
    let checksum_body = render_json(&serde_json::json!({
        "files": serde_json::Value::Object(files),
        "package": package_digest,
    }))?;
    std::fs::write(
        vendor_dir.join(".cargo-checksum.json"),
        checksum_body.as_bytes(),
    )?;
    let index_body = render_json(&serde_json::json!({
        "files": entries
            .iter()
            .map(|(base, digest)| serde_json::json!({"name": base, "sha256": digest}))
            .collect::<Vec<_>>(),
        "name": crate_name,
        "version": version,
    }))?;
    std::fs::write(registry_dir.join("index.json"), index_body.as_bytes())?;
    Ok(house)
}

pub(crate) const CRATES_GATE: OwnerGate = OwnerGate {
    tool: "crates_deploy",
    action: "publish",
    credentials: &["CRATES_API_TOKEN"],
    approved: "CRATES_PUBLISH_APPROVED",
};

const CARGO_CHILD_ENV_EXTRA: &[&str] = &["CARGO_HOME", "RUSTUP_HOME"];

const CARGO_FAILURE_LINES: usize = 40;

fn crates_publish_staged(
    program: &Path,
    files: &[std::path::PathBuf],
    staging: &Path,
    crate_name: &str,
    version: &str,
    token: &str,
) -> Result<(), CratesError> {
    let mapped = crates_package_paths(files).map_err(CratesError::Paths)?;
    stage_mapped(&mapped, staging).map_err(CratesError::Stage)?;
    let manifest = staging.join(manifest_name());
    if !manifest.is_file() {
        return Err(CratesError::StagedNoManifest);
    }
    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    manifest_version(&text, crate_name, version)?;
    cargo_publish(program, &manifest, token)
}

fn cargo_publish(program: &Path, manifest: &Path, token: &str) -> Result<(), CratesError> {
    let mut cmd = scrubbed_command(
        program,
        &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
        &[("CARGO_REGISTRY_TOKEN", token)],
    );
    cmd.arg("publish").arg("--manifest-path").arg(manifest);
    let output = run_publish(&mut cmd).map_err(CratesError::Run)?;
    if output.success() {
        return Ok(());
    }
    Err(CratesError::CargoFailed {
        command: "cargo publish",
        status: exit_text(&output),
        lines: cargo_failure_lines(&output.stderr, token),
    })
}

fn cargo_failure_lines(stderr: &[u8], token: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(stderr);
    let text = if token.is_empty() {
        text.into_owned()
    } else {
        text.replace(token, "[redacted]")
    };
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(CARGO_FAILURE_LINES))
        .map(|line| (*line).to_owned())
        .collect()
}

fn manifest_identity(text: &str) -> (Option<String>, Option<String>) {
    let (mut name, mut version) = (None, None);
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.split('#').next().unwrap_or("").trim();
        let plain = value
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .map(str::to_owned);
        match (in_package, key.trim()) {
            (true, "name") => name = plain,
            (true, "version") => version = plain,
            _ => {}
        }
    }
    (name, version)
}

fn manifest_version(text: &str, crate_name: &str, version: &str) -> Result<String, CratesError> {
    let (name, found) = manifest_identity(text);
    if name.as_deref() != Some(crate_name) {
        return Err(CratesError::NameMismatch {
            found: name,
            target: crate_name.to_owned(),
        });
    }
    resolve_version("crates_deploy", manifest_name(), found.as_deref(), version)
        .map_err(CratesError::Refused)
}

pub(crate) fn crates_version(
    files: &[std::path::PathBuf],
    crate_name: &str,
    version: &str,
) -> Result<String, CratesError> {
    let mapped = crates_package_paths(files).map_err(CratesError::Paths)?;
    let Some((manifest, _)) = mapped.iter().find(|(_, rel)| rel == manifest_name()) else {
        return Err(CratesError::NoManifest);
    };
    let text = path_io::read_to_string(manifest).map_err(CratesError::Io)?;
    manifest_version(&text, crate_name, version)
}

fn crates_package_check(
    cargo: Option<&Path>,
    mapped: &[(std::path::PathBuf, String)],
) -> Result<(), CratesError> {
    let Some(program) = cargo else {
        println!("crates_deploy: cargo not found on PATH; skipped cargo package --list check");
        return Ok(());
    };
    let staging = tempfile_stage_dir("crates-check-").map_err(CratesError::TempDir)?;
    let checked = stage_mapped(mapped, &staging)
        .map_err(CratesError::Stage)
        .and_then(|()| cargo_package_list(program, &staging.join(manifest_name())));
    dx_atomic_fs::discard_dir(&staging);
    checked
}

fn cargo_package_list(program: &Path, manifest: &Path) -> Result<(), CratesError> {
    let mut cmd = scrubbed_command(
        program,
        &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
        &[],
    );
    cmd.args([
        "package",
        "--list",
        "--offline",
        "--allow-dirty",
        "--manifest-path",
    ])
    .arg(manifest);
    let output = run_publish(&mut cmd).map_err(CratesError::Run)?;
    if output.success() {
        return Ok(());
    }
    Err(CratesError::CargoFailed {
        command: "cargo package --list",
        status: exit_text(&output),
        lines: cargo_failure_lines(&output.stderr, ""),
    })
}

fn index_path(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", name.get(..1).unwrap_or_default()),
        _ => format!(
            "{}/{}/{name}",
            name.get(..2).unwrap_or_default(),
            name.get(2..4).unwrap_or_default()
        ),
    }
}

pub(crate) const CRATES_INDEX: &str = "https://index.crates.io";
pub(crate) const CRATES_DL: &str = "https://static.crates.io/crates";
const CRATES_FETCH_LIMIT: u64 = 64 << 20;

fn published_below(crate_name: &str, next: &str) -> Result<Option<String>, ApiCheckError> {
    let url = format!("{CRATES_INDEX}/{}", index_path(crate_name));
    Ok(fetch(&url, CRATES_FETCH_LIMIT)?
        .and_then(|body| latest_published(&String::from_utf8_lossy(&body), next)))
}

pub(crate) fn fetch_published_crate(
    index: &str,
    dl: &str,
    name: &str,
    version: &str,
    files: &[std::path::PathBuf],
    dest_dir: &Path,
) -> Result<std::path::PathBuf, CratesError> {
    let mapped = crates_package_paths(files).map_err(CratesError::Paths)?;
    let not_published = || CratesError::NotPublished {
        name: name.to_owned(),
        version: version.to_owned(),
    };
    let index_url = format!("{index}/{}", index_path(name));
    let body = fetch(&index_url, CRATES_FETCH_LIMIT)
        .map_err(CratesError::Fetch)?
        .unwrap_or_default();
    let expected = String::from_utf8_lossy(&body)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|entry| entry.get("vers").and_then(serde_json::Value::as_str) == Some(version))
        .and_then(|entry| {
            entry
                .get("cksum")
                .and_then(serde_json::Value::as_str)
                .map(str::to_ascii_lowercase)
        })
        .ok_or_else(not_published)?;
    path_io::create_dir_all(dest_dir).map_err(CratesError::Io)?;
    let file_name = format!("{name}-{version}.crate");
    let dest = dest_dir.join(&file_name);
    let checked = fetch_to(
        &format!("{dl}/{name}/{version}/download"),
        &dest,
        CRATES_FETCH_LIMIT,
    )
    .map_err(CratesError::Fetch)
    .and_then(|found| if found { Ok(()) } else { Err(not_published()) })
    .and_then(|()| {
        let got =
            dx_digest::sha256_file_hex(&dest).map_err(|source| CratesError::InvalidCrate {
                file_name: file_name.clone(),
                source,
            })?;
        if got == expected {
            Ok(())
        } else {
            Err(CratesError::ChecksumMismatch {
                file_name: file_name.clone(),
                expected,
                got,
            })
        }
    })
    .and_then(|()| check_crate_members(&dest, &file_name, &format!("{name}-{version}/"), &mapped));
    match checked {
        Ok(()) => Ok(dest),
        Err(error) => {
            if let Err(cleanup) = std::fs::remove_file(&dest) {
                dx_output::write_stderr(
                    &mut io::stderr(),
                    format_args!("crates_deploy: remove {}: {cleanup}\n", dest.display()),
                );
            }
            Err(error)
        }
    }
}

fn check_crate_members(
    path: &Path,
    file_name: &str,
    prefix: &str,
    mapped: &[(std::path::PathBuf, String)],
) -> Result<(), CratesError> {
    let invalid = |source| CratesError::InvalidCrate {
        file_name: file_name.to_owned(),
        source,
    };
    let mut pinned = std::collections::HashMap::new();
    for (src, rel) in mapped {
        let digest = dx_digest::sha256_file_hex(src).map_err(CratesError::Paths)?;
        let key = if rel == manifest_name() {
            format!("{}.orig", manifest_name())
        } else {
            rel.clone()
        };
        pinned.insert(key, digest);
    }
    let generated = [
        manifest_name(),
        Ecosystem::Cargo.lockfile(),
        ".cargo_vcs_info.json",
    ];
    let open = std::fs::File::open(path).map_err(invalid)?;
    let mut foreign: Option<(String, &'static str)> = None;
    dx_archive::visit(
        open,
        dx_archive::Format::TarGz,
        dx_archive::Limits::default(),
        |member, kind, body| {
            if foreign.is_some() {
                return Ok(());
            }
            let Some(rel) = member.strip_prefix(prefix) else {
                foreign = Some((member.to_owned(), "is outside the crate directory"));
                return Ok(());
            };
            match kind {
                dx_archive::EntryKind::Dir => return Ok(()),
                dx_archive::EntryKind::File => {}
                dx_archive::EntryKind::Symlink
                | dx_archive::EntryKind::HardLink
                | dx_archive::EntryKind::Other => {
                    foreign = Some((member.to_owned(), "is not a regular file"));
                    return Ok(());
                }
            }
            if generated.contains(&rel) {
                return Ok(());
            }
            match pinned.get(rel) {
                None => foreign = Some((member.to_owned(), "is not a pinned source file")),
                Some(want) if *want != dx_digest::sha256_reader_hex(body)? => {
                    foreign = Some((member.to_owned(), "differs from the pinned source file"));
                }
                Some(_) => {}
            }
            Ok(())
        },
    )
    .map_err(invalid)?;
    match foreign {
        Some((member, reason)) => Err(CratesError::ForeignMember {
            file_name: file_name.to_owned(),
            member,
            reason,
        }),
        None => Ok(()),
    }
}

fn semver_changes(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("--- failure "))
        .map(|line| line.trim_end_matches(" ---").to_owned())
        .collect()
}

fn crates_api_check(
    program: &Path,
    mapped: &[(std::path::PathBuf, String)],
    version: &str,
    crate_name: &str,
    baseline: Baseline,
    crate_dir: &str,
) -> Result<Outcome, CratesError> {
    let mut export_dir = None;
    let (previous, baseline_args) = if baseline == Baseline::Git {
        let tag = match previous_git_tag(&workspace_dir(), version) {
            Ok(Some(tag)) => tag,
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error.to_string())),
        };
        let dest = tempfile_stage_dir("crates-api-base-").map_err(CratesError::TempDir)?;
        match export_git_dir(&workspace_dir(), crate_dir, &tag, &dest, manifest_name()) {
            Ok(true) => {}
            Ok(false) => {
                dx_atomic_fs::discard_dir(&dest);
                return Ok(Outcome::Skipped(format!(
                    "no {} at {crate_dir} in {tag}",
                    manifest_name()
                )));
            }
            Err(error) => {
                dx_atomic_fs::discard_dir(&dest);
                return Err(CratesError::BaselineExport(error));
            }
        }
        let root = dest.display().to_string();
        export_dir = Some(dest);
        (
            tag.trim_start_matches('v').to_owned(),
            vec!["--baseline-root".to_owned(), root],
        )
    } else {
        match published_below(crate_name, version) {
            Ok(Some(published)) => (
                published.clone(),
                vec!["--baseline-version".to_owned(), published],
            ),
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error.to_string())),
        }
    };
    let staging = match tempfile_stage_dir("crates-api-") {
        Ok(dir) => dir,
        Err(error) => {
            if let Some(dir) = &export_dir {
                dx_atomic_fs::discard_dir(dir);
            }
            return Err(CratesError::TempDir(error));
        }
    };
    let ran = stage_mapped(mapped, &staging)
        .map_err(CratesError::Stage)
        .and_then(|()| {
            let mut cmd = scrubbed_command(
                program,
                &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
                &[],
            );
            cmd.args(["semver-checks", "check-release", "--manifest-path"])
                .arg(staging.join(manifest_name()))
                .args(&baseline_args);
            run_publish(&mut cmd).map_err(CratesError::Run)
        });
    dx_atomic_fs::discard_dir(&staging);
    if let Some(dir) = &export_dir {
        dx_atomic_fs::discard_dir(dir);
    }
    let output = ran?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let changes = if output.success() {
        Vec::new()
    } else {
        changes_or_failure(semver_changes(&text), false, "cargo-semver-checks", &text)
    };
    Ok(Outcome::Compared { previous, changes })
}

fn crates_api_report(
    semver_checks: Option<&Path>,
    mapped: &[(std::path::PathBuf, String)],
    (crate_name, version): (&str, &str),
    (mode, baseline): (ApiCheckMode, Baseline),
    crate_dir: &str,
) -> Result<(), CratesError> {
    if mode == ApiCheckMode::Off {
        return Ok(());
    }
    let outcome = match semver_checks {
        Some(program) => {
            crates_api_check(program, mapped, version, crate_name, baseline, crate_dir)?
        }
        None => Outcome::Skipped("cargo-semver-checks is not available".to_owned()),
    };
    conclude("crates_deploy", mode, version, outcome).map_err(CratesError::ApiCheck)
}

pub fn crates_main(
    crate_rlocs: &[String],
    crate_name: &str,
    version: &str,
    argv: &[String],
) -> i32 {
    crates_main_checked(
        crate_rlocs,
        crate_name,
        version,
        &CratesApiCheck {
            api_compat: "off",
            api_baseline: "registry",
            semver_checks_rloc: "",
            crate_dir: "",
        },
        argv,
    )
}

pub struct CratesApiCheck<'a> {
    pub api_compat: &'a str,
    pub api_baseline: &'a str,
    pub semver_checks_rloc: &'a str,
    pub crate_dir: &'a str,
}

pub fn crates_main_checked(
    crate_rlocs: &[String],
    crate_name: &str,
    version: &str,
    api: &CratesApiCheck,
    argv: &[String],
) -> i32 {
    let CratesApiCheck {
        api_compat,
        api_baseline,
        semver_checks_rloc,
        crate_dir,
    } = *api;
    let Some(run) = DeployRun::start(
        "crates_deploy",
        "the vendor tree is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let mut files = Vec::new();
    for rloc in crate_rlocs.iter().filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    let version = match crates_version(&files, crate_name, version) {
        Ok(version) => version,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let version = version.as_str();
    let (Some(mode), Some(baseline)) = (
        ApiCheckMode::parse(api_compat),
        Baseline::parse(api_baseline),
    ) else {
        eprintln!(
            "crates_deploy: invalid api_compat '{api_compat}' or api_baseline '{api_baseline}'"
        );
        return 1;
    };
    let semver_checks = if semver_checks_rloc.is_empty() {
        None
    } else {
        run.resolve(semver_checks_rloc)
    };
    let live = OwnerGate::requested(env_or_empty);
    let checked = crates_package_paths(&files)
        .map_err(CratesError::Paths)
        .and_then(|mapped| {
            crates_api_report(
                semver_checks.as_deref(),
                &mapped,
                (crate_name, version),
                (mode, baseline),
                crate_dir,
            )?;
            if live {
                return Ok(());
            }
            crates_package_check(which_on_path("cargo").as_deref(), &mapped)
        });
    if let Err(message) = checked {
        eprintln!("{message}");
        return 1;
    }
    if live {
        if std::env::var("CRATES_ALLOW_DIRTY").unwrap_or_default() == "1" {
            eprintln!(
                "crates_deploy: --allow-dirty rejected by default; publish from a clean tree instead"
            );
            return 1;
        }
        if let Some(error) = placeholder_refusal("crates_deploy", version) {
            eprintln!("{error}");
            return 1;
        }
        let secrets = match CRATES_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let staging = match tempfile_stage_dir("crates-publish-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("crates_deploy: {error}");
                return 1;
            }
        };
        let published = crates_publish_staged(
            Path::new("cargo"),
            &files,
            &staging,
            crate_name,
            version,
            secrets.get("CRATES_API_TOKEN"),
        );
        dx_atomic_fs::discard_dir(&staging);
        match published {
            Ok(()) => {
                println!("crates_deploy: published {crate_name} {version}");
                return 0;
            }
            Err(message) => {
                eprintln!("{message}");
                return 1;
            }
        }
    }
    match crates_build_vendor(&files, outdir, crate_name, version) {
        Ok(house) => {
            let build_mode = deploy_build_mode();
            println!("crates_deploy: staged {crate_name} {version} (build_mode: {build_mode})");
            println!(
                "  vendor: {}",
                house.join("vendor").join(crate_name).display()
            );
            println!(
                "  registry: {}",
                house
                    .join("registry")
                    .join(crate_name)
                    .join(version)
                    .display()
            );
            0
        }
        Err(error) => {
            eprintln!("crates_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::basename;
    use crate::test_support::*;

    #[test]
    fn manifest_must_match_target() {
        let ok = "[package]\nname = \"demo\"\nversion = \"1.2.3\" # x\n[dependencies]\nname = \"other\"\n";
        assert_eq!(
            manifest_version(ok, "demo", "1.2.3").expect("matches"),
            "1.2.3"
        );
        assert_eq!(manifest_version(ok, "demo", "").expect("reads"), "1.2.3");
        let wrong_name = manifest_version(ok, "other", "").expect_err("name refused");
        assert!(
            matches!(&wrong_name, CratesError::NameMismatch { found: Some(found), target }
            if found == "demo" && target == "other")
        );
        assert_eq!(
            wrong_name.to_string(),
            format!(
                "crates_deploy: {} is demo, target is other; refusing",
                manifest_name()
            )
        );
        let mismatch = manifest_version(ok, "demo", "2.0.0").expect_err("version refused");
        assert!(matches!(mismatch, CratesError::Refused(_)));
        assert!(
            mismatch
                .to_string()
                .contains("declares version 1.2.3, BUILD declares 2.0.0")
        );
        let workspace = "[package]\nname = \"demo\"\nversion.workspace = true\n";
        assert!(manifest_version(workspace, "demo", "").is_err());
        assert!(manifest_version("", "demo", "").is_err());
    }

    #[test]
    fn crates_vendor_builds_checksums() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join(manifest_name());
        std::fs::write(
            &manifest,
            b"[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\n",
        )
        .expect("write manifest");
        let source = scratch.path().join("lib.rs");
        std::fs::write(&source, b"pub fn hello() -> &str { \"hello\" }\n").expect("write src");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let files = vec![manifest.clone(), source.clone()];
        let house = crates_build_vendor(&files, &outdir, "crates_demo", "0.0.0").expect("vendor");
        let checksum = std::fs::read_to_string(
            house
                .join("vendor")
                .join("crates_demo")
                .join(".cargo-checksum.json"),
        )
        .expect("checksum");
        let pairs = files
            .iter()
            .map(|src| {
                (
                    basename(src).expect("base"),
                    dx_digest::sha256_file_hex(src).expect("hex"),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert!(checksum.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(&checksum).expect("checksum parses");
        let want_files = serde_json::json!({
            manifest_name(): pairs[manifest_name()],
            "lib.rs": pairs["lib.rs"],
        });
        assert_eq!(parsed["files"], want_files);
        let package = parsed["package"].as_str().expect("package");
        assert_eq!(package.len(), 64);
        assert!(package.bytes().all(|b| b.is_ascii_hexdigit()));
        let index = std::fs::read_to_string(
            house
                .join("registry")
                .join("crates_demo")
                .join("0.0.0")
                .join("index.json"),
        )
        .expect("index");
        assert!(index.ends_with('\n'));
        let index: serde_json::Value = serde_json::from_str(&index).expect("index parses");
        assert_eq!(index["name"], "crates_demo");
        assert_eq!(index["version"], "0.0.0");
        assert_eq!(index["files"][0]["name"], manifest_name());
        assert_eq!(index["files"][0]["sha256"], pairs[manifest_name()].as_str());
        assert!(crates_build_vendor(&[], &outdir, "crates_demo", "0.0.0").is_err());
    }

    #[test]
    fn stage_mapped_copies_and_verifies_each_file() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mapped = crates_package_paths(&files).expect("paths");
        let staging = scratch.path().join("staging");
        stage_mapped(&mapped, &staging).expect("stage");
        for (src, rel) in &mapped {
            assert_eq!(
                std::fs::read(staging.join(rel)).expect("staged"),
                std::fs::read(src).expect("source"),
            );
        }
        let source = scratch.path().join("missing.rs");
        let missing = vec![(source.clone(), "missing.rs".to_owned())];
        let error = stage_mapped(&missing, &staging).expect_err("missing source");
        assert_eq!(error.path, source);
        assert_eq!(
            error.to_string(),
            format!("cannot stage {}", source.display())
        );
    }

    #[test]
    fn crates_version_reports_an_unreadable_manifest() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join(format!("pkg/{}", manifest_name()));
        let error = crates_version(std::slice::from_ref(&manifest), "crates_demo", "")
            .expect_err("unreadable");
        assert!(matches!(&error, CratesError::Io(source) if source.path() == manifest));
        assert!(
            error.to_string().starts_with(&format!(
                "crates_deploy: cannot read {}",
                manifest.display()
            )),
            "{error}"
        );
    }

    #[test]
    fn crates_publish_staged_failure_is_reported_for_caller_cleanup() {
        let scratch = scratch_dir();
        let staging = scratch.path().join("crates-publish-x");
        let missing = vec![scratch.path().join(format!("missing/{}", manifest_name()))];
        let error = crates_publish_staged(
            Path::new("cargo"),
            &missing,
            &staging,
            "crates_demo",
            "0.0.0",
            "t",
        )
        .expect_err("stage fails");
        assert!(matches!(error, CratesError::Stage(_)));
        assert!(
            error
                .to_string()
                .starts_with("crates_deploy: cannot stage ")
        );
    }

    fn nested_crate(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                manifest_name(),
                "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
            ),
            write_crate_file(root, "src/lib.rs", "pub mod parser;\npub mod lexer;\n"),
            write_crate_file(root, "src/parser/mod.rs", "pub fn parse() -> u8 { 1 }\n"),
            write_crate_file(root, "src/lexer/mod.rs", "pub fn lex() -> u8 { 2 }\n"),
        ]
    }

    #[test]
    fn crates_vendor_preserves_package_paths() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let outdir = scratch.path().join("out");
        let house = crates_build_vendor(&files, &outdir, "crates_demo", "0.0.0").expect("vendor");
        for base in [
            house.join("vendor/crates_demo"),
            house.join("registry/crates_demo/0.0.0"),
        ] {
            for rel in [
                manifest_name(),
                "src/lib.rs",
                "src/parser/mod.rs",
                "src/lexer/mod.rs",
            ] {
                assert!(base.join(rel).is_file(), "{}", base.join(rel).display());
            }
        }
        let checksum =
            std::fs::read_to_string(house.join("vendor/crates_demo/.cargo-checksum.json"))
                .expect("checksum");
        assert!(checksum.contains("\"src/parser/mod.rs\""), "{checksum}");
        assert!(checksum.contains("\"src/lexer/mod.rs\""), "{checksum}");
    }

    #[test]
    fn crates_vendor_keeps_explicit_manifest_paths() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let files = vec![
            write_crate_file(
                &root,
                manifest_name(),
                "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\n[lib]\npath = \"custom/entry.rs\"\n",
            ),
            write_crate_file(&root, "custom/entry.rs", "pub fn hello() {}\n"),
        ];
        let house =
            crates_build_vendor(&files, &scratch.path().join("out"), "crates_demo", "0.0.0")
                .expect("vendor");
        assert!(house.join("vendor/crates_demo/custom/entry.rs").is_file());
    }

    #[test]
    fn crates_package_paths_reject_collisions_and_escapes() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let manifest = write_crate_file(&root, manifest_name(), "[package]\n");
        let outside = write_crate_file(scratch.path(), "other/lib.rs", "");
        let error = crates_package_paths(&[manifest.clone(), outside]).expect_err("outside");
        assert!(
            error.to_string().contains("outside the package root"),
            "{error}"
        );
        let dotdot = root.join("src/../lib.rs");
        write_crate_file(&root, "lib.rs", "");
        let error = crates_package_paths(&[manifest.clone(), dotdot]).expect_err("dotdot");
        assert!(error.to_string().contains("unsafe package path"), "{error}");
        let error =
            crates_package_paths(&[manifest.clone(), root.join("lib.rs"), root.join("lib.rs")])
                .expect_err("dup");
        assert!(error.to_string().contains("path collision"), "{error}");
        let file = write_crate_file(&root, "src", "");
        let nested = write_crate_file(&root, "src_dir/x.rs", "");
        assert!(crates_package_paths(&[manifest.clone(), file, nested]).is_ok());
        let file = root.join("src");
        let child = root.join("src/lib.rs");
        let error = crates_package_paths(&[manifest.clone(), file, child]).expect_err("file+dir");
        assert!(
            error.to_string().contains("both a file and a directory"),
            "{error}"
        );
        let error = crates_package_paths(&[write_crate_file(&root, "lib.rs", "")])
            .expect_err("no manifest");
        assert!(
            error
                .to_string()
                .contains(&format!("no {}", manifest_name())),
            "{error}"
        );
        let error = crates_package_paths(&[
            write_crate_file(&root, &format!("a/{}", manifest_name()), ""),
            write_crate_file(&root, &format!("b/{}", manifest_name()), ""),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
    }

    #[test]
    fn cargo_failure_lines_keep_last_lines_and_hide_token() {
        let stderr: String = (1..=50).fold(String::new(), |mut text, n| {
            text.push_str("line ");
            text.push_str(&n.to_string());
            text.push_str(" s3cret\n");
            text
        });
        let lines = cargo_failure_lines(stderr.as_bytes(), "s3cret");
        assert_eq!(lines.len(), 40);
        assert_eq!(lines[0], "line 11 [redacted]");
        assert_eq!(lines[39], "line 50 [redacted]");
    }

    #[cfg(unix)]
    #[test]
    fn cargo_publish_shows_cargo_reason() {
        let scratch = scratch_dir();
        let fake = scratch.path().join("cargo");
        dx_testing::executable_script(
            &fake,
            "#!/bin/sh\necho 'error: crate version 0.0.0 is already uploaded' >&2\necho \"token $CARGO_REGISTRY_TOKEN\" >&2\nexit 101\n",
        );
        let error = cargo_publish(&fake, &scratch.path().join(manifest_name()), "s3cret")
            .expect_err("publish fails");
        assert!(matches!(
            error,
            CratesError::CargoFailed {
                command: "cargo publish",
                ..
            }
        ));
        assert!(!format!("{error:?}").contains("s3cret"));
        let message = error.to_string();
        assert!(
            message.contains("cargo publish failed with exit status: 101"),
            "{message}"
        );
        assert!(
            message.contains("\ncargo: error: crate version 0.0.0 is already uploaded"),
            "{message}"
        );
        assert!(message.contains("\ncargo: token [redacted]"), "{message}");
        assert!(!message.contains("s3cret"), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn package_check_fails_with_cargo_message_and_skips_without_cargo() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mapped = crates_package_paths(&files).expect("paths");
        let fake = scratch.path().join("cargo");
        dx_testing::executable_script(
            &fake,
            "#!/bin/sh\necho 'error: no license or license-file' >&2\nexit 101\n",
        );
        let error = crates_package_check(Some(&fake), &mapped).expect_err("rejected");
        assert!(matches!(
            error,
            CratesError::CargoFailed {
                command: "cargo package --list",
                ..
            }
        ));
        let message = error.to_string();
        assert!(
            message.contains("cargo: error: no license or license-file"),
            "{message}"
        );
        std::fs::write(&fake, "#!/bin/sh\nexit 0\n").expect("rewrite");
        assert!(crates_package_check(Some(&fake), &mapped).is_ok());
        assert!(crates_package_check(None, &mapped).is_ok());
    }

    #[test]
    fn crates_vendor_builds_with_cargo_consumer() {
        let Ok(probe) = std::process::Command::new("cargo")
            .arg("--version")
            .output()
        else {
            return;
        };
        if !probe.status.success() {
            return;
        }
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let house =
            crates_build_vendor(&files, &scratch.path().join("out"), "crates_demo", "0.0.0")
                .expect("vendor");
        let consumer = scratch.path().join("consumer");
        write_crate_file(
            &consumer,
            manifest_name(),
            "[package]\nname = \"consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[dependencies]\ncrates_demo = \"0.0.0\"\n",
        );
        write_crate_file(
            &consumer,
            "src/main.rs",
            "fn main() { println!(\"{}\", crates_demo::parser::parse() + crates_demo::lexer::lex()); }\n",
        );
        write_crate_file(
            &consumer,
            ".cargo/config.toml",
            &format!(
                "[source.crates-io]\nreplace-with = \"vendored\"\n[source.vendored]\ndirectory = \"{}\"\n",
                house
                    .join("vendor")
                    .display()
                    .to_string()
                    .replace('\\', "/")
            ),
        );
        let output = std::process::Command::new("cargo")
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", scratch.path().join("target"))
            .env("CARGO_HOME", scratch.path().join("cargo_home"))
            .output()
            .expect("cargo run");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");
    }

    #[cfg(unix)]
    #[test]
    fn api_check_fails_on_breaking_change_unless_major() {
        let scratch = scratch_dir();
        let repo = scratch.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        write_crate_file(
            &repo,
            &format!("pkg/{}", manifest_name()),
            "[package]\nname = \"pkg\"\n",
        );
        write_crate_file(&repo, "pkg/src/lib.rs", "pub fn hello() {}\n");
        for args in [
            &["init", "-q"][..],
            &["add", "."],
            &["commit", "-q", "-m", "x"],
            &["tag", "v1.2.0"],
        ] {
            dx_test_scratch::git(&repo, &repo, args).unwrap();
        }
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mapped = crates_package_paths(&files).expect("paths");
        let args_log = scratch.path().join("args");
        let exported = scratch.path().join("exported");
        let fake = scratch.path().join("cargo-semver-checks");
        dx_testing::executable_script(
            &fake,
            &format!(
                "#!/bin/sh\necho \"$@\" > {}\nwhile [ $# -gt 0 ]; do if [ \"$1\" = --baseline-root ]; then cp \"$2/{}\" {}; fi; shift; done\necho '--- failure pub_function_missing: pub fn removed or renamed ---'\nexit 1\n",
                args_log.display(),
                manifest_name(),
                exported.display()
            ),
        );
        let repo_dir = repo.display().to_string();
        let run_in = |dir: &str, version: &str, mode: ApiCheckMode| {
            with_env(&[("BUILD_WORKSPACE_DIRECTORY", Some(&repo_dir))], || {
                crates_api_report(
                    Some(&fake),
                    &mapped,
                    ("pkg", version),
                    (mode, Baseline::Git),
                    dir,
                )
            })
        };
        let run = |version: &str, mode: ApiCheckMode| run_in("pkg", version, mode);
        let refused = run("1.2.1", ApiCheckMode::Fail).expect_err("patch bump refused");
        assert!(matches!(refused, CratesError::ApiCheck(_)));
        assert!(refused.to_string().contains("1.2.0 -> 1.2.1"), "{refused}");
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(logged.contains("--baseline-root"), "{logged}");
        assert!(!logged.contains("--baseline-rev"), "{logged}");
        assert_eq!(
            std::fs::read_to_string(&exported).expect("exported manifest"),
            "[package]\nname = \"pkg\"\n"
        );
        assert!(run("2.0.0", ApiCheckMode::Fail).is_ok());
        assert!(run("1.2.1", ApiCheckMode::Warn).is_ok());
        std::fs::remove_file(&args_log).expect("clear log");
        assert!(run("1.2.1", ApiCheckMode::Off).is_ok());
        assert!(!args_log.exists(), "off must not run the tool");
        assert!(run_in("moved", "2.0.0", ApiCheckMode::Fail).is_ok());
        assert!(
            !args_log.exists(),
            "a crate missing at the tag skips the tool"
        );
    }

    #[test]
    fn api_check_with_the_pinned_semver_checks_binary() {
        let pinned = std::env::var("CARGO_SEMVER_CHECKS").unwrap_or_default();
        let cargo = std::env::var("DX_TEST_CARGO").unwrap_or_default();
        if pinned.is_empty() || cargo.is_empty() {
            println!("skipped: run under Bazel for the pinned cargo-semver-checks and toolchain");
            return;
        }
        if env_test_in_child("crates::tests::api_check_with_the_pinned_semver_checks_binary") {
            return;
        }
        let tool = resolve_runfile(&pinned).expect("pinned tool");
        let cargo = resolve_runfile(&cargo).expect("cargo");
        let rustc =
            resolve_runfile(&std::env::var("DX_TEST_RUSTC").expect("rustc")).expect("rustc");
        let rustdoc =
            resolve_runfile(&std::env::var("DX_TEST_RUSTDOC").expect("rustdoc")).expect("rustdoc");
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        for bin in [&cargo, &rustc, &rustdoc] {
            let dir = bin.parent().expect("tool dir").to_path_buf();
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        if let Some(old) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&old));
        }
        let path = std::env::join_paths(dirs).expect("PATH");
        let path = path.to_str().expect("PATH is UTF-8").to_string();
        let scratch = scratch_dir();
        let repo = scratch.path().join("repo");
        let manifest = "[package]\nname = \"pkg\"\nversion = \"1.2.0\"\nedition = \"2021\"\n";
        write_crate_file(&repo, &format!("pkg/{}", manifest_name()), manifest);
        write_crate_file(&repo, "pkg/src/lib.rs", "pub fn hello() {}\n");
        for args in [
            &["init", "-q"][..],
            &["add", "."],
            &["commit", "-q", "-m", "x"],
            &["tag", "v1.2.0"],
        ] {
            dx_test_scratch::git(&repo, &repo, args).unwrap();
        }
        let current = scratch.path().join("in/pkg");
        let files = vec![
            write_crate_file(&current, manifest_name(), manifest),
            write_crate_file(&current, "src/lib.rs", "\n"),
        ];
        let mapped = crates_package_paths(&files).expect("paths");
        let repo_dir = repo.display().to_string();
        let cargo_home = scratch.path().join("cargo-home").display().to_string();
        let cargo = cargo.display().to_string();
        let rustc = rustc.display().to_string();
        let rustdoc = rustdoc.display().to_string();
        let env = [
            ("BUILD_WORKSPACE_DIRECTORY", Some(repo_dir.as_str())),
            ("PATH", Some(path.as_str())),
            ("CARGO", Some(cargo.as_str())),
            ("RUSTC", Some(rustc.as_str())),
            ("RUSTDOC", Some(rustdoc.as_str())),
            ("CARGO_HOME", Some(cargo_home.as_str())),
            ("CARGO_NET_OFFLINE", Some("true")),
        ];
        let run = |version: &str| {
            with_env(&env, || {
                crates_api_report(
                    Some(&tool),
                    &mapped,
                    ("pkg", version),
                    (ApiCheckMode::Fail, Baseline::Git),
                    "pkg",
                )
            })
        };
        let refused = run("1.2.1").expect_err("patch bump refused");
        assert!(matches!(refused, CratesError::ApiCheck(_)));
        assert!(refused.to_string().contains("1.2.0 -> 1.2.1"), "{refused}");
        assert!(run("2.0.0").is_ok());
    }

    #[test]
    fn semver_changes_lists_failed_lints() {
        let out = "--- failure pub_fn_missing: pub fn removed ---\nnote\n  --- failure enum_variant_added: variant added ---\n";
        assert_eq!(
            semver_changes(out),
            vec![
                "pub_fn_missing: pub fn removed",
                "enum_variant_added: variant added"
            ]
        );
    }

    #[cfg(unix)]
    fn published_crate(members: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = dx_archive::TarWriter::new();
        for (rel, body) in members {
            tar.add_file(&format!("crates_demo-0.0.0/{rel}"), body.as_bytes(), 0o644)
                .expect("append");
        }
        tar.finish().expect("finish tar.gz")
    }

    #[cfg(unix)]
    fn crates_registry(body: Vec<u8>, index_version: &str, cksum: Option<&str>) -> String {
        let cksum = cksum.map_or_else(|| dx_digest::sha256_hex(&body), str::to_owned);
        static_registry(|_| {
            vec![
                (
                    format!("/index/{}", index_path("crates_demo")),
                    format!(
                        "{{\"name\":\"crates_demo\",\"vers\":\"{index_version}\",\"cksum\":\"{cksum}\"}}\n"
                    )
                    .into_bytes(),
                ),
                ("/dl/crates_demo/0.0.0/download".to_owned(), body),
            ]
        })
    }

    #[cfg(unix)]
    fn fetch_demo(
        registry: &str,
        files: &[std::path::PathBuf],
        dest: &Path,
    ) -> Result<std::path::PathBuf, CratesError> {
        fetch_published_crate(
            &format!("{registry}/index"),
            &format!("{registry}/dl"),
            "crates_demo",
            "0.0.0",
            files,
            dest,
        )
    }

    #[cfg(unix)]
    fn pinned_members(manifest: &str) -> Vec<(&str, &str)> {
        vec![
            (
                manifest_name(),
                "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\nedition = \"2021\"\n# normalized\n",
            ),
            ("Cargo.toml.orig", manifest),
            (Ecosystem::Cargo.lockfile(), "version = 4\n"),
            (".cargo_vcs_info.json", "{}\n"),
            ("src/lib.rs", "pub mod parser;\npub mod lexer;\n"),
            ("src/parser/mod.rs", "pub fn parse() -> u8 { 1 }\n"),
        ]
    }

    #[cfg(unix)]
    const DEMO_MANIFEST: &str =
        "[package]\nname = \"crates_demo\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

    #[cfg(unix)]
    #[test]
    fn fetch_published_crate_keeps_a_crate_of_pinned_sources() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let body = published_crate(&pinned_members(DEMO_MANIFEST));
        let registry = crates_registry(body.clone(), "0.0.0", None);
        let dest = scratch.path().join("out");
        let path = fetch_demo(&registry, &files, &dest).expect("fetched");
        assert_eq!(path, dest.join("crates_demo-0.0.0.crate"));
        assert_eq!(std::fs::read(&path).expect("crate"), body);
    }

    #[cfg(unix)]
    #[test]
    fn fetch_published_crate_refuses_a_checksum_mismatch() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let body = published_crate(&pinned_members(DEMO_MANIFEST));
        let registry = crates_registry(body, "0.0.0", Some(&"0".repeat(64)));
        let dest = scratch.path().join("out");
        let error = fetch_demo(&registry, &files, &dest).expect_err("mismatch");
        assert!(
            matches!(error, CratesError::ChecksumMismatch { .. }),
            "{error}"
        );
        assert!(!dest.join("crates_demo-0.0.0.crate").exists());
    }

    #[cfg(unix)]
    #[test]
    fn fetch_published_crate_refuses_a_version_missing_from_the_index() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let body = published_crate(&pinned_members(DEMO_MANIFEST));
        let registry = crates_registry(body, "0.0.1", None);
        let error =
            fetch_demo(&registry, &files, &scratch.path().join("out")).expect_err("missing");
        assert!(matches!(error, CratesError::NotPublished { .. }), "{error}");
        assert!(error.to_string().contains("crates_demo 0.0.0"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn fetch_published_crate_refuses_an_extra_member() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mut members = pinned_members(DEMO_MANIFEST);
        members.push(("build.rs", "fn main() {}\n"));
        let registry = crates_registry(published_crate(&members), "0.0.0", None);
        let error = fetch_demo(&registry, &files, &scratch.path().join("out")).expect_err("extra");
        let CratesError::ForeignMember { member, reason, .. } = &error else {
            panic!("{error}");
        };
        assert_eq!(member, "crates_demo-0.0.0/build.rs");
        assert_eq!(*reason, "is not a pinned source file");
    }

    #[cfg(unix)]
    #[test]
    fn fetch_published_crate_refuses_a_changed_manifest() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let changed = format!("{DEMO_MANIFEST}build = \"build.rs\"\n");
        let registry = crates_registry(published_crate(&pinned_members(&changed)), "0.0.0", None);
        let error =
            fetch_demo(&registry, &files, &scratch.path().join("out")).expect_err("changed");
        let CratesError::ForeignMember { member, reason, .. } = &error else {
            panic!("{error}");
        };
        assert_eq!(member, "crates_demo-0.0.0/Cargo.toml.orig");
        assert_eq!(*reason, "differs from the pinned source file");
    }
}
