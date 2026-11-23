use crate::api_compat::{
    ApiCompatError, ApiCompatMode, Baseline, Outcome, changes_or_failure, conclude, fetch_to_with,
    fetch_with, highest_below, previous_git_tag, workspace_dir,
};
use crate::archive::*;
use crate::run::{
    DOTNET_CHILD_ENV_EXTRA, DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode,
    env_or_empty, exit_text, print_redacted, run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum NugetError {
    InvalidVersion {
        text: String,
        reason: &'static str,
    },
    VersionPartTooLarge {
        text: String,
        source: std::num::ParseIntError,
    },
    NuspecXml(quick_xml::Error),
    NuspecText(quick_xml::encoding::EncodingError),
    NuspecEntity,
    NuspecMissing(&'static str),
    NuspecEmpty(&'static str),
    NuspecDuplicate(&'static str),
    Open {
        path: PathBuf,
        source: io::Error,
    },
    NotZip {
        path: PathBuf,
        source: io::Error,
    },
    NoNuspec(PathBuf),
    ManyNuspecs {
        path: PathBuf,
        names: Vec<String>,
    },
    ReadNuspec {
        name: String,
        source: io::Error,
    },
    NuspecGone {
        name: String,
        path: PathBuf,
    },
    NuspecUtf8 {
        name: String,
        source: std::string::FromUtf8Error,
    },
    InNuspec {
        name: String,
        source: Box<NugetError>,
    },
    InvalidId {
        name: String,
        id: String,
    },
    IdMismatch {
        packaged: String,
        declared: String,
    },
    VersionMismatch {
        packaged: String,
        declared: String,
    },
    InvalidBaseline(String),
    TempDir(io::Error),
    ApiCompatHome(PathIoError),
    ApiCompatRun(RunError),
    ApiCompat(ApiCompatError),
}

impl fmt::Display for NugetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion { text, reason } => {
                write!(f, "invalid NuGet version {text:?}: {reason}")
            }
            Self::VersionPartTooLarge { text, source } => {
                write!(
                    f,
                    "invalid NuGet version {text:?}: version part too large: {source}"
                )
            }
            Self::NuspecXml(source) => write!(f, "nuspec is not valid XML: {source}"),
            Self::NuspecText(source) => write!(f, "nuspec text is not valid: {source}"),
            Self::NuspecEntity => f.write_str("nuspec id or version contains an entity reference"),
            Self::NuspecMissing(name) => write!(f, "nuspec has no <{name}> in <metadata>"),
            Self::NuspecEmpty(name) => write!(f, "nuspec <{name}> is empty"),
            Self::NuspecDuplicate(name) => {
                write!(f, "nuspec has more than one <{name}> in <metadata>")
            }
            Self::Open { path, source } => write!(f, "cannot open {}: {source}", path.display()),
            Self::NotZip { path, source } => {
                write!(
                    f,
                    "{} is not a valid nupkg archive: {source}",
                    path.display()
                )
            }
            Self::NoNuspec(path) => write!(f, "{} has no root .nuspec", path.display()),
            Self::ManyNuspecs { path, names } => write!(
                f,
                "{} has more than one root .nuspec: {}",
                path.display(),
                names.join(", ")
            ),
            Self::ReadNuspec { name, source } => write!(f, "cannot read {name}: {source}"),
            Self::NuspecGone { name, path } => {
                write!(f, "cannot read {name}: missing from {}", path.display())
            }
            Self::NuspecUtf8 { name, source } => write!(f, "cannot read {name}: {source}"),
            Self::InNuspec { name, source } => write!(f, "{name}: {source}"),
            Self::InvalidId { name, id } => write!(f, "{name}: invalid NuGet package id {id:?}"),
            Self::IdMismatch { packaged, declared } => write!(
                f,
                "package id mismatch: nupkg declares {packaged}, BUILD declares {declared}"
            ),
            Self::VersionMismatch { packaged, declared } => write!(
                f,
                "package version mismatch: nupkg declares {packaged}, BUILD declares {declared}"
            ),
            Self::InvalidBaseline(baseline) => {
                write!(f, "nuget_deploy: invalid api_baseline '{baseline}'")
            }
            Self::TempDir(source) => write!(f, "nuget_deploy: {source}"),
            Self::ApiCompatHome(source) => write!(f, "nuget_deploy: ApiCompat: {source}"),
            Self::ApiCompatRun(source) => write!(f, "nuget_deploy: ApiCompat: {source}"),
            Self::ApiCompat(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for NugetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::VersionPartTooLarge { source, .. } => Some(source),
            Self::NuspecXml(source) => Some(source),
            Self::NuspecText(source) => Some(source),
            Self::Open { source, .. }
            | Self::NotZip { source, .. }
            | Self::ReadNuspec { source, .. } => Some(source),
            Self::TempDir(source) => Some(source),
            Self::NuspecUtf8 { source, .. } => Some(source),
            Self::InNuspec { source, .. } => Some(source.as_ref()),
            Self::ApiCompatHome(source) => Some(source),
            Self::ApiCompatRun(source) => Some(source),
            Self::ApiCompat(source) => Some(source),
            Self::InvalidVersion { .. }
            | Self::NuspecEntity
            | Self::NuspecMissing(_)
            | Self::NuspecEmpty(_)
            | Self::NuspecDuplicate(_)
            | Self::NoNuspec(_)
            | Self::ManyNuspecs { .. }
            | Self::NuspecGone { .. }
            | Self::InvalidId { .. }
            | Self::IdMismatch { .. }
            | Self::VersionMismatch { .. }
            | Self::InvalidBaseline(_) => None,
        }
    }
}

pub fn nuget_normalize_version(raw: &str) -> Result<String, NugetError> {
    let text = raw.trim();
    let without_meta = text.split('+').next().unwrap_or("");
    let (core, release) = match without_meta.split_once('-') {
        Some((core, release)) => (core, Some(release)),
        None => (without_meta, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() > 4 {
        return Err(NugetError::InvalidVersion {
            text: text.to_owned(),
            reason: "want at most four numeric parts",
        });
    }
    let mut numbers = [0u64; 4];
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(NugetError::InvalidVersion {
                text: text.to_owned(),
                reason: "want numeric version parts",
            });
        }
        *slot = part
            .parse()
            .map_err(|source| NugetError::VersionPartTooLarge {
                text: text.to_owned(),
                source,
            })?;
    }
    let mut out = format!("{}.{}.{}", numbers[0], numbers[1], numbers[2]);
    if numbers[3] != 0 {
        out.push_str(&format!(".{}", numbers[3]));
    }
    if let Some(release) = release {
        let valid = !release.is_empty()
            && release.split('.').all(|label| {
                !label.is_empty()
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            });
        if !valid {
            return Err(NugetError::InvalidVersion {
                text: text.to_owned(),
                reason: "bad prerelease label",
            });
        }
        out.push('-');
        out.push_str(&release.to_ascii_lowercase());
    }
    Ok(out)
}

fn nuget_parse_nuspec(xml: &str) -> Result<(String, String), NugetError> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut id: Vec<String> = Vec::new();
    let mut version: Vec<String> = Vec::new();
    loop {
        let event = reader.read_event().map_err(NugetError::NuspecXml)?;
        match event {
            Event::Start(start) => {
                stack.push(String::from_utf8_lossy(start.local_name().as_ref()).into_owned());
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    list.push(String::new());
                }
            }
            Event::Empty(start) => {
                stack.push(String::from_utf8_lossy(start.local_name().as_ref()).into_owned());
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    list.push(String::new());
                }
                stack.pop();
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(text) => {
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    let text = text.xml10_content().map_err(NugetError::NuspecText)?;
                    if let Some(last) = list.last_mut() {
                        last.push_str(&text);
                    }
                }
            }
            Event::CData(text) => {
                if let Some(list) = nuget_identity_slot(&stack, &mut id, &mut version) {
                    let text = text.decode().map_err(NugetError::NuspecText)?;
                    if let Some(last) = list.last_mut() {
                        last.push_str(&text);
                    }
                }
            }
            Event::GeneralRef(_) => {
                if nuget_identity_slot(&stack, &mut id, &mut version).is_some() {
                    return Err(NugetError::NuspecEntity);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    let one = |name: &'static str, values: Vec<String>| -> Result<String, NugetError> {
        match values.as_slice() {
            [] => Err(NugetError::NuspecMissing(name)),
            [value] => {
                let value = value.trim().to_owned();
                if value.is_empty() {
                    Err(NugetError::NuspecEmpty(name))
                } else {
                    Ok(value)
                }
            }
            _ => Err(NugetError::NuspecDuplicate(name)),
        }
    };
    Ok((one("id", id)?, one("version", version)?))
}

fn nuget_identity_slot<'a>(
    stack: &[String],
    id: &'a mut Vec<String>,
    version: &'a mut Vec<String>,
) -> Option<&'a mut Vec<String>> {
    match stack {
        [a, b, c] if a == "package" && b == "metadata" && c == "id" => Some(id),
        [a, b, c] if a == "package" && b == "metadata" && c == "version" => Some(version),
        _ => None,
    }
}

pub fn nuget_read_identity(nupkg: &Path) -> Result<(String, String), NugetError> {
    let file = std::fs::File::open(nupkg).map_err(|source| NugetError::Open {
        path: nupkg.to_path_buf(),
        source,
    })?;
    let names: Vec<String> = dx_archive::zip_names(file)
        .map_err(|source| NugetError::NotZip {
            path: nupkg.to_path_buf(),
            source,
        })?
        .into_iter()
        .filter(|name| !name.contains('/') && name.to_ascii_lowercase().ends_with(".nuspec"))
        .collect();
    let name = match names.as_slice() {
        [] => return Err(NugetError::NoNuspec(nupkg.to_path_buf())),
        [one] => one.clone(),
        _ => {
            return Err(NugetError::ManyNuspecs {
                path: nupkg.to_path_buf(),
                names,
            });
        }
    };
    let in_nuspec = |source| NugetError::InNuspec {
        name: name.clone(),
        source: Box::new(source),
    };
    let xml = dx_archive::read_zip_member(nupkg, &name, 1 << 20)
        .map_err(|source| NugetError::ReadNuspec {
            name: name.clone(),
            source,
        })?
        .ok_or_else(|| NugetError::NuspecGone {
            name: name.clone(),
            path: nupkg.to_path_buf(),
        })?;
    let xml = String::from_utf8(xml).map_err(|source| NugetError::NuspecUtf8 {
        name: name.clone(),
        source,
    })?;
    let (id, version) = nuget_parse_nuspec(&xml).map_err(in_nuspec)?;
    if id.len() > 100
        || id.starts_with(['.', '-', '_'])
        || id.ends_with(['.', '-', '_'])
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(NugetError::InvalidId { name, id });
    }
    let version = nuget_normalize_version(&version).map_err(in_nuspec)?;
    Ok((id, version))
}

pub fn nuget_check_identity(
    actual: &(String, String),
    expected_id: &str,
    expected_version: &str,
) -> Result<(), NugetError> {
    if !expected_id.is_empty() && !actual.0.eq_ignore_ascii_case(expected_id) {
        return Err(NugetError::IdMismatch {
            packaged: actual.0.clone(),
            declared: expected_id.to_owned(),
        });
    }
    if !expected_version.is_empty() {
        let want = nuget_normalize_version(expected_version)?;
        if want != actual.1 {
            return Err(NugetError::VersionMismatch {
                packaged: actual.1.clone(),
                declared: expected_version.to_owned(),
            });
        }
    }
    Ok(())
}

pub fn nuget_build_feed(
    nupkg_src: &Path,
    outdir: &Path,
    feed_name: &str,
) -> io::Result<std::path::PathBuf> {
    let feed = outdir.join(format!("{feed_name}-feed"));
    std::fs::create_dir_all(&feed)?;
    let base = basename(nupkg_src)?;
    let dest = feed.join(&base);
    copy_verified(nupkg_src, &dest)?;
    Ok(feed)
}

pub(crate) const NUGET_GATE: OwnerGate = OwnerGate {
    tool: "nuget_deploy",
    action: "push",
    credentials: &["NUGET_API_TOKEN"],
    approved: "NUGET_PUBLISH_APPROVED",
};

const DUPLICATE_NOTICE: &str = "already exists";

fn is_duplicate_notice(output: &str) -> bool {
    output.to_lowercase().contains(DUPLICATE_NOTICE)
}

fn push_result(duplicate: bool, id: &str, version: &str, source: &str) -> String {
    if duplicate {
        format!("nuget_deploy: skipped {id} {version}: already on {source}")
    } else {
        format!("nuget_deploy: pushed {id} {version}")
    }
}

const NUGET_ENV_KEY_MIN: (u32, u32) = (7, 6);

fn nuget_parse_version(output: &str) -> Option<(u32, u32)> {
    let last = output.lines().rev().find(|line| !line.trim().is_empty())?;
    last.split_whitespace().find_map(|token| {
        let mut parts = token.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor_raw = parts.next()?;
        let digits: String = minor_raw.chars().take_while(char::is_ascii_digit).collect();
        Some((major, digits.parse::<u32>().ok()?))
    })
}

pub fn nuget_reads_api_key_env(version_output: &str) -> bool {
    nuget_parse_version(version_output).is_some_and(|version| version >= NUGET_ENV_KEY_MIN)
}

pub fn nuget_live_push(
    dotnet: &Path,
    nupkg: &Path,
    source: &str,
    token: &str,
    id: &str,
    version: &str,
) -> i32 {
    let mut probe = scrubbed_command(
        dotnet,
        &[PUBLISH_CHILD_ENV_KEEP, DOTNET_CHILD_ENV_EXTRA],
        &[
            ("DOTNET_NOLOGO", "1"),
            ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
        ],
    );
    probe.arg("nuget").arg("--version");
    let reported = match run_publish(&mut probe) {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            return 1;
        }
    };
    if !nuget_reads_api_key_env(&reported) {
        eprintln!(
            "nuget_deploy: NuGet {} does not read NUGET_API_KEY; need NuGet {}.{}+ (.NET SDK 10.0.300+); refusing to put the key on the command line",
            reported.lines().last().unwrap_or("").trim(),
            NUGET_ENV_KEY_MIN.0,
            NUGET_ENV_KEY_MIN.1
        );
        return 1;
    }
    let mut cmd = scrubbed_command(
        dotnet,
        &[PUBLISH_CHILD_ENV_KEEP, DOTNET_CHILD_ENV_EXTRA],
        &[("NUGET_API_KEY", token)],
    );
    cmd.arg("nuget")
        .arg("push")
        .arg(nupkg)
        .arg("--source")
        .arg(source)
        .arg("--skip-duplicate");
    match run_publish(&mut cmd) {
        Ok(out) if out.success() => {
            print_redacted(&out, token);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let duplicate = is_duplicate_notice(&stdout) || is_duplicate_notice(&stderr);
            println!("{}", push_result(duplicate, id, version, source));
            0
        }
        Ok(out) => {
            print_redacted(&out, token);
            eprintln!("nuget_deploy: dotnet failed with {}", exit_text(&out));
            1
        }
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            1
        }
    }
}

pub struct NugetApiCheck<'a> {
    pub mode: &'a str,
    pub baseline: &'a str,
    pub dotnet_rloc: &'a str,
    pub apicompat_rloc: &'a str,
}

fn nuget_package_base(service_index: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(service_index).ok()?;
    let id = json
        .get("resources")?
        .as_array()?
        .iter()
        .find(|resource| {
            resource
                .get("@type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind.starts_with("PackageBaseAddress/3.0.0"))
        })?
        .get("@id")?
        .as_str()?;
    Some(format!("{}/", id.trim_end_matches('/')))
}

fn apicompat_changes(output: &str) -> Vec<String> {
    let mut changes: Vec<String> = Vec::new();
    let mut open = false;
    for line in output.lines() {
        let code = line.trim_start();
        let is_diagnostic = code.len() > 7
            && code.starts_with("CP")
            && code
                .get(2..6)
                .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()))
            && code.as_bytes().get(6) == Some(&b':');
        if is_diagnostic {
            changes.push(code.trim_end().to_owned());
            open = true;
        } else if open && line.starts_with(char::is_whitespace) && !code.is_empty() {
            if let Some(last) = changes.last_mut() {
                last.push(' ');
                last.push_str(code.trim_end());
            }
        } else {
            open = false;
        }
    }
    changes
}

fn nuget_api_check(
    client: &dx_http::Client,
    (dotnet, apicompat): (&Path, &Path),
    (package_id, version): (&str, &str),
    (nupkg, source): (&Path, &str),
    baseline: &str,
    workspace: &Path,
) -> Result<Outcome, NugetError> {
    let baseline = Baseline::parse(baseline)
        .ok_or_else(|| NugetError::InvalidBaseline(baseline.to_owned()))?;
    let index = match fetch_with(client, source, 16 << 20) {
        Ok(Some(body)) => body,
        Ok(None) => return Ok(Outcome::Skipped(format!("{source} not found"))),
        Err(error) => return Ok(Outcome::Skipped(error.to_string())),
    };
    let Some(base) = nuget_package_base(&String::from_utf8_lossy(&index)) else {
        return Ok(Outcome::Skipped(format!(
            "{source} lists no PackageBaseAddress/3.0.0"
        )));
    };
    let id = package_id.to_ascii_lowercase();
    let previous = if baseline == Baseline::Git {
        match previous_git_tag(workspace, version) {
            Ok(Some(tag)) => tag.trim_start_matches('v').to_owned(),
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error.to_string())),
        }
    } else {
        let url = format!("{base}{id}/index.json");
        let body = match fetch_with(client, &url, 16 << 20) {
            Ok(Some(body)) => body,
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error.to_string())),
        };
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        let versions = json
            .get("versions")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten();
        match highest_below(versions.filter_map(|v| v.as_str()), version) {
            Some(previous) => previous,
            None => return Ok(Outcome::FirstRelease),
        }
    };
    let lower = previous.to_ascii_lowercase();
    let url = format!("{base}{id}/{lower}/{id}.{lower}.nupkg");
    let work = tempfile_stage_dir("nuget-api-").map_err(NugetError::TempDir)?;
    let old_nupkg = work.join(format!("{id}.{lower}.nupkg"));
    let skipped = match fetch_to_with(client, &url, &old_nupkg, 512 << 20) {
        Ok(true) => None,
        Ok(false) => Some(format!("{url} not found")),
        Err(error) => Some(error.to_string()),
    };
    if let Some(reason) = skipped {
        dx_atomic_fs::discard_dir(&work);
        return Ok(Outcome::Skipped(reason));
    }
    let home = work.join("home");
    let output = path_io::create_dir_all(&home)
        .map_err(NugetError::ApiCompatHome)
        .and_then(|()| {
            let home = home.display().to_string();
            run_publish(
                scrubbed_command(
                    dotnet,
                    &[PUBLISH_CHILD_ENV_KEEP],
                    &[
                        ("DOTNET_ROLL_FORWARD", "Major"),
                        ("DOTNET_CLI_HOME", &home),
                        ("DOTNET_NOLOGO", "1"),
                        ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                        ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
                    ],
                )
                .arg("exec")
                .arg(apicompat)
                .arg("package")
                .arg(nupkg)
                .arg("--baseline-package")
                .arg(&old_nupkg),
            )
            .map_err(NugetError::ApiCompatRun)
        });
    dx_atomic_fs::discard_dir(&work);
    let output = output?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let success = output.success();
    Ok(Outcome::Compared {
        previous,
        changes: changes_or_failure(apicompat_changes(&text), success, "ApiCompat", &text),
    })
}

pub fn nuget_main(
    nupkg_rloc: &str,
    target_name: &str,
    expected_id: &str,
    expected_version: &str,
    package_source: &str,
    argv: &[String],
) -> i32 {
    let off = NugetApiCheck {
        mode: "off",
        baseline: "registry",
        dotnet_rloc: "",
        apicompat_rloc: "",
    };
    nuget_main_checked(
        nupkg_rloc,
        target_name,
        expected_id,
        expected_version,
        package_source,
        &off,
        argv,
    )
}

pub fn nuget_main_checked(
    nupkg_rloc: &str,
    target_name: &str,
    expected_id: &str,
    expected_version: &str,
    package_source: &str,
    api: &NugetApiCheck,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "nuget_deploy",
        "the feed is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(nupkg) = run.resolve(nupkg_rloc) else {
        return 1;
    };
    let identity = match nuget_read_identity(&nupkg).and_then(|identity| {
        nuget_check_identity(&identity, expected_id, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("nuget_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package_id, package_version) = (identity.0.as_str(), identity.1.as_str());
    let Some(mode) = ApiCompatMode::parse(api.mode) else {
        eprintln!("nuget_deploy: invalid api_compat '{}'", api.mode);
        return 1;
    };
    if mode != ApiCompatMode::Off {
        let tools = if api.dotnet_rloc.is_empty() || api.apicompat_rloc.is_empty() {
            None
        } else {
            run.resolve(api.dotnet_rloc)
                .zip(run.resolve(api.apicompat_rloc))
        };
        let outcome = match tools {
            Some((dotnet, apicompat)) => match crate::http::client() {
                Ok(client) => nuget_api_check(
                    &client,
                    (&dotnet, &apicompat),
                    (package_id, package_version),
                    (&nupkg, package_source),
                    api.baseline,
                    &workspace_dir(),
                ),
                Err(error) => Ok(Outcome::Skipped(error.to_string())),
            },
            None => Ok(Outcome::Skipped(
                "the .NET toolchain or ApiCompat is not available".to_owned(),
            )),
        };
        if let Err(error) = outcome.and_then(|outcome| {
            conclude("nuget_deploy", mode, package_version, outcome).map_err(NugetError::ApiCompat)
        }) {
            eprintln!("{error}");
            return 1;
        }
    }
    if OwnerGate::requested(env_or_empty) {
        if package_version == "0.0.0" {
            eprintln!(
                "nuget_deploy: live push refuses {package_id} version {package_version}; set a real version in the nuspec"
            );
            return 1;
        }
        let secrets = match NUGET_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(dotnet) = which_on_path("dotnet") else {
            eprintln!("nuget_deploy: 'dotnet' CLI not found on PATH");
            return 1;
        };
        return nuget_live_push(
            &dotnet,
            &nupkg,
            package_source,
            secrets.get("NUGET_API_TOKEN"),
            package_id,
            package_version,
        );
    }
    match nuget_build_feed(&nupkg, outdir, target_name) {
        Ok(feed) => {
            let build_mode = deploy_build_mode();
            println!(
                "nuget_deploy: staged {package_id} {package_version} (build_mode: {build_mode})"
            );
            println!("  feed: {}", feed.display());
            if let Ok(base) = basename(&nupkg) {
                println!("  nupkg: {}", feed.join(base).display());
            }
            0
        }
        Err(error) => {
            eprintln!("nuget_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn deploy_env<R>(live: bool, f: impl FnOnce() -> R) -> R {
        let mut vars: Vec<(&str, Option<&str>)> = vec![
            ("RUNFILES_DIR", None),
            ("RUNFILES_MANIFEST_FILE", None),
            ("NUGET_API_TOKEN", None),
            ("NUGET_PUBLISH_APPROVED", None),
            ("DX_DEPLOY_LIVE", None),
        ];
        if live {
            vars[2] = ("NUGET_API_TOKEN", Some("secret-token"));
            vars[3] = ("NUGET_PUBLISH_APPROVED", Some("1"));
            vars[4] = ("DX_DEPLOY_LIVE", Some("1"));
        }
        with_env(&vars, f)
    }

    const APICOMPAT_SAMPLE: &str = "\
API compatibility errors between 'lib/net10.0/Demo.dll' (demo.1.0.0.nupkg) and 'lib/net10.0/Demo.dll' (demo.1.1.0.nupkg):
CP0002: Member 'void Demo.Api.Hello()' exists on [Baseline] lib/net10.0/Demo.dll but not on lib/net10.0/Demo.dll
CP0002: Member 'int Demo.Api.Count(string)' exists on [Baseline] lib/net10.0/Demo.dll but not on lib/net10.0/Demo.dll
CP0006: Cannot add interface member 'void Demo.IThing.Stop()' to lib/net10.0/Demo.dll because it does not exist on [Baseline] lib/net10.0/Demo.dll
API breaking changes found. If those are intentional, the APICompat suppression file can be updated by specifying the '--generate-suppression-file' parameter.
";

    #[test]
    fn apicompat_output_counts_one_change_per_diagnostic() {
        let changes = apicompat_changes(APICOMPAT_SAMPLE);
        assert_eq!(changes.len(), 3);
        assert!(changes[2].starts_with("CP0006: Cannot add interface member"));
        assert!(changes[2].ends_with("[Baseline] lib/net10.0/Demo.dll"));
        let wrapped =
            "CP0002: Member 'A.B()' exists on [Baseline]\n    a.dll but not on a.dll\nnext line\n";
        assert_eq!(
            apicompat_changes(wrapped),
            vec!["CP0002: Member 'A.B()' exists on [Baseline] a.dll but not on a.dll".to_owned()]
        );
        assert!(
            apicompat_changes("APICompat ran successfully without finding any breaking changes.\n")
                .is_empty()
        );
    }

    #[test]
    fn package_base_comes_from_the_service_index() {
        let index = r#"{"version":"3.0.0","resources":[{"@id":"https://x/query","@type":"SearchQueryService"},{"@id":"https://x/flat","@type":"PackageBaseAddress/3.0.0"}]}"#;
        assert_eq!(
            nuget_package_base(index).as_deref(),
            Some("https://x/flat/")
        );
        assert_eq!(nuget_package_base(r#"{"resources":[]}"#), None);
        assert_eq!(nuget_package_base("<html>"), None);
    }

    #[cfg(unix)]
    #[test]
    fn api_check_fails_a_breaking_minor_and_passes_a_major_against_the_registry() {
        let scratch = scratch_dir();
        let registry = static_registry(|origin| {
            vec![
                (
                    "/v3/index.json".to_owned(),
                    format!(r#"{{"resources":[{{"@id":"{origin}/flat","@type":"PackageBaseAddress/3.0.0"}}]}}"#).into_bytes(),
                ),
                (
                    "/flat/demo/index.json".to_owned(),
                    br#"{"versions":["0.9.0","1.0.0","2.0.0-beta.1"]}"#.to_vec(),
                ),
                (
                    "/flat/demo/1.0.0/demo.1.0.0.nupkg".to_owned(),
                    b"old nupkg".to_vec(),
                ),
            ]
        });
        let args_log = scratch.path().join("args");
        let env_log = scratch.path().join("env");
        let fake = scratch.path().join("dotnet");
        dx_testing::executable_script(
            &fake,
            &format!(
                "#!/bin/sh\necho \"$@\" > {}\nenv > {}\necho \"CP0002: Member 'void Demo.Api.Hello()' exists on [Baseline] lib/net10.0/Demo.dll but not on lib/net10.0/Demo.dll\"\nexit 1\n",
                args_log.display(),
                env_log.display()
            ),
        );
        let client = crate::http::test_client();
        let source = format!("{registry}/v3/index.json");
        let run = |id: &str, version: &str| {
            nuget_api_check(
                &client,
                (&fake, Path::new("apicompat.dll")),
                (id, version),
                (Path::new("new.nupkg"), &source),
                "registry",
                scratch.path(),
            )
            .expect("ran")
        };
        let outcome = run("Demo", "1.1.0");
        assert_eq!(
            outcome,
            Outcome::Compared {
                previous: "1.0.0".to_owned(),
                changes: vec!["CP0002: Member 'void Demo.Api.Hello()' exists on [Baseline] lib/net10.0/Demo.dll but not on lib/net10.0/Demo.dll".to_owned()],
            }
        );
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(
            logged.starts_with("exec apicompat.dll package new.nupkg --baseline-package ")
                && logged.trim_end().ends_with("demo.1.0.0.nupkg"),
            "{logged}"
        );
        let env = std::fs::read_to_string(&env_log).expect("env");
        assert!(env.contains("DOTNET_ROLL_FORWARD=Major"), "{env}");
        assert!(!env.contains("NUGET_API_KEY"), "{env}");
        assert!(conclude("nuget_deploy", ApiCompatMode::Fail, "1.1.0", outcome).is_err());
        let major = run("Demo", "2.0.0");
        assert!(conclude("nuget_deploy", ApiCompatMode::Fail, "2.0.0", major).is_ok());
        assert_eq!(run("Missing", "1.0.0"), Outcome::FirstRelease);
    }

    #[test]
    fn nuget_feed_copies_nupkg() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("nuget_demo.0.0.0.nupkg");
        std::fs::write(&nupkg, b"nuget-demo-nupkg-bytes-v1").expect("write nupkg");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let feed = nuget_build_feed(&nupkg, &outdir, "nuget_demo").expect("feed");
        assert!(feed.join("nuget_demo.0.0.0.nupkg").is_file());
    }

    #[test]
    fn nuget_version_normalization_follows_nuget() {
        let norm = |v: &str| nuget_normalize_version(v).expect("version");
        assert_eq!(norm("1.0"), "1.0.0");
        assert_eq!(norm("01.2.3.0"), "1.2.3");
        assert_eq!(norm("1.2.3.4"), "1.2.3.4");
        assert_eq!(norm("1.0.0-BETA.1+build5"), "1.0.0-beta.1");
        assert_eq!(norm("0.0.0.0+meta"), "0.0.0");
        assert!(nuget_normalize_version("1.x.0").is_err());
        assert!(nuget_normalize_version("1.2.3.4.5").is_err());
        assert!(nuget_normalize_version("1.0.0-").is_err());
        assert!(nuget_normalize_version("").is_err());
    }

    #[test]
    fn nuget_identity_reads_the_packaged_nuspec() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("anything.nupkg");
        write_nupkg(
            &nupkg,
            &[
                ("Dx.Plugin.nuspec", &nuspec("Dx.Plugin", "1.2.0.0")),
                ("lib/a.dll", "x"),
            ],
        );
        let identity = nuget_read_identity(&nupkg).expect("identity");
        assert_eq!(identity, ("Dx.Plugin".to_owned(), "1.2.0".to_owned()));
        assert!(nuget_check_identity(&identity, "dx.plugin", "1.2").is_ok());
        assert!(nuget_check_identity(&identity, "", "").is_ok());
        let id_error = nuget_check_identity(&identity, "Other", "").expect_err("id");
        assert!(
            matches!(id_error, NugetError::IdMismatch { .. }),
            "{id_error:?}"
        );
        assert_eq!(
            id_error.to_string(),
            "package id mismatch: nupkg declares Dx.Plugin, BUILD declares Other"
        );
        let version_error = nuget_check_identity(&identity, "", "2.0.0").expect_err("version");
        assert!(
            matches!(version_error, NugetError::VersionMismatch { .. }),
            "{version_error:?}"
        );
        assert!(
            version_error
                .to_string()
                .contains("package version mismatch"),
            "{version_error}"
        );
    }

    #[test]
    fn nuget_identity_rejects_bad_packages() {
        let scratch = scratch_dir();
        let path = scratch.path().join("bad.nupkg");
        std::fs::write(&path, b"not a zip").expect("write");
        assert!(
            nuget_read_identity(&path)
                .expect_err("zip")
                .to_string()
                .contains("not a valid nupkg")
        );
        write_nupkg(&path, &[("lib/a.dll", "x")]);
        assert!(
            nuget_read_identity(&path)
                .expect_err("none")
                .to_string()
                .contains("no root .nuspec")
        );
        write_nupkg(
            &path,
            &[
                ("a.nuspec", &nuspec("A", "1.0.0")),
                ("b.nuspec", &nuspec("B", "1.0.0")),
            ],
        );
        assert!(
            nuget_read_identity(&path)
                .expect_err("two")
                .to_string()
                .contains("more than one root .nuspec")
        );
        write_nupkg(
            &path,
            &[(
                "a.nuspec",
                "<package><metadata><id>A</id></metadata></package>",
            )],
        );
        assert!(
            nuget_read_identity(&path)
                .expect_err("no version")
                .to_string()
                .contains("no <version>")
        );
        write_nupkg(
            &path,
            &[(
                "a.nuspec",
                "<package><metadata><id>A</id><id>B</id><version>1.0.0</version></metadata></package>",
            )],
        );
        assert!(
            nuget_read_identity(&path)
                .expect_err("dup")
                .to_string()
                .contains("more than one <id>")
        );
        write_nupkg(&path, &[("a.nuspec", "<package><metadata><id>A</id>")]);
        assert!(nuget_read_identity(&path).is_err());
        write_nupkg(&path, &[("a.nuspec", &nuspec("A/../B", "1.0.0"))]);
        assert!(
            nuget_read_identity(&path)
                .expect_err("id chars")
                .to_string()
                .contains("invalid NuGet package id")
        );
        write_nupkg(&path, &[("a.nuspec", &nuspec("A", "one"))]);
        assert!(
            nuget_read_identity(&path)
                .expect_err("version")
                .to_string()
                .contains("invalid NuGet version")
        );
    }

    #[test]
    fn nuget_main_stages_and_reports_the_packaged_identity() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("pkg.nupkg");
        write_nupkg(&nupkg, &[("Real.Pkg.nuspec", &nuspec("Real.Pkg", "2.0.0"))]);
        let outdir = scratch.path().join("out");
        let argv = vec!["launcher".to_owned(), outdir.display().to_string()];
        let rloc = nupkg.display().to_string();
        deploy_env(false, || {
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "Real.Pkg",
                    "2.0",
                    "https://nuget.invalid",
                    &argv
                ),
                0
            );
            assert_eq!(
                nuget_main(&rloc, "publish", "", "", "https://nuget.invalid", &argv),
                0
            );
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "publish",
                    "",
                    "https://nuget.invalid",
                    &argv
                ),
                1
            );
            assert_eq!(
                nuget_main(
                    &rloc,
                    "publish",
                    "",
                    "9.9.9",
                    "https://nuget.invalid",
                    &argv
                ),
                1
            );
        });
        let staged = outdir.join("publish-feed/pkg.nupkg");
        assert_eq!(
            std::fs::read(staged).expect("staged"),
            std::fs::read(&nupkg).expect("src")
        );
    }

    #[test]
    fn nuget_live_push_checks_the_actual_package_version() {
        let scratch = scratch_dir();
        let nupkg = scratch.path().join("placeholder.nupkg");
        write_nupkg(&nupkg, &[("P.nuspec", &nuspec("P", "0.0.0"))]);
        let argv = vec![
            "launcher".to_owned(),
            scratch.path().join("out").display().to_string(),
        ];
        let rloc = nupkg.display().to_string();
        deploy_env(true, || {
            assert_eq!(
                nuget_main(&rloc, "p", "", "1.0.0", "https://nuget.invalid", &argv),
                1,
                "a configured version cannot hide the packaged version"
            );
            assert_eq!(
                nuget_main(&rloc, "p", "", "", "https://nuget.invalid", &argv),
                1
            );
        });
    }

    #[cfg(unix)]
    fn fake_dotnet(dir: &Path, nuget_version: &str, exit: i32) -> std::path::PathBuf {
        let path = dir.join("dotnet");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nif [ \"$2\" = \"--version\" ]; then printf 'NuGet Command Line\\n{nuget_version}\\n'; exit 0; fi\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\nexit {exit}\n"
        );
        dx_testing::executable_script(&path, &script);
        path
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_keeps_the_key_out_of_argv() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "7.6.0", 0);
        with_env(&[("DX_LEAK_PROBE", Some("1"))], || {
            assert_eq!(
                nuget_live_push(
                    &dotnet,
                    Path::new("a.nupkg"),
                    "https://n.invalid",
                    "sekret",
                    "a",
                    "1.0.0"
                ),
                0
            );
        });
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(args.contains("push") && args.contains("--skip-duplicate"));
        assert!(!args.contains("sekret") && !args.contains("--api-key"));
        assert!(env.contains("NUGET_API_KEY=sekret"));
        assert!(!env.contains("DX_LEAK_PROBE"));
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_refuses_nuget_older_than_7_6() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "6.14.0", 0);
        assert_eq!(
            nuget_live_push(
                &dotnet,
                Path::new("a.nupkg"),
                "https://n.invalid",
                "sekret",
                "a",
                "1.0.0"
            ),
            1
        );
        assert!(!scratch.path().join("args.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn nuget_live_push_surfaces_tool_failure() {
        let scratch = scratch_dir();
        let dotnet = fake_dotnet(scratch.path(), "7.6.0", 1);
        assert_eq!(
            nuget_live_push(
                &dotnet,
                Path::new("a.nupkg"),
                "https://n.invalid",
                "sekret",
                "a",
                "1.0.0"
            ),
            1
        );
    }

    #[test]
    fn nuget_duplicate_push_prints_skipped() {
        assert!(is_duplicate_notice(
            "Conflict - The package Already Exists on the feed"
        ));
        assert!(!is_duplicate_notice("Your package was pushed."));
        assert_eq!(
            push_result(true, "a", "1.0.0", "https://n.invalid"),
            "nuget_deploy: skipped a 1.0.0: already on https://n.invalid"
        );
        assert_eq!(
            push_result(false, "a", "1.0.0", "https://n.invalid"),
            "nuget_deploy: pushed a 1.0.0"
        );
    }

    #[test]
    fn nuget_api_key_env_needs_nuget_7_6() {
        assert!(!nuget_reads_api_key_env("6.12.1"));
        assert!(!nuget_reads_api_key_env("7.5.9"));
        assert!(nuget_reads_api_key_env("7.6.0"));
        assert!(nuget_reads_api_key_env("7.6.0-preview.1"));
        assert!(nuget_reads_api_key_env("NuGet Command Line\n7.6.0\n"));
        assert!(nuget_reads_api_key_env("8.0"));
        assert!(!nuget_reads_api_key_env(
            "Welcome to .NET 9.0!\nSDK Version: 9.0.100\n\nNuGet Command Line\n6.12.1\n"
        ));
        assert!(!nuget_reads_api_key_env(""));
        assert!(!nuget_reads_api_key_env("garbage"));
    }
}
