use crate::api_compat::{
    ApiCheckError, ApiCheckMode, Baseline, Outcome, changes_or_failure, conclude, fetch, fetch_to,
    highest_below, previous_git_tag, workspace_dir,
};
use crate::archive::*;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PairMismatch {
    pub wheel: String,
    pub project: String,
    pub version: String,
    pub sdist: String,
    pub sdist_project: String,
    pub sdist_version: String,
}

#[derive(Debug)]
pub enum PypiError {
    InvalidWheelName(String),
    InvalidSdistName(String),
    UnsupportedArtifact(String),
    InvalidIdentity(String),
    Basename(io::Error),
    OpenWheel {
        name: String,
        source: io::Error,
    },
    NotZip {
        name: String,
        source: io::Error,
    },
    NoRecord(String),
    ReadMember {
        member: String,
        name: String,
        source: io::Error,
    },
    MissingMember {
        member: String,
        name: String,
    },
    MemberUtf8 {
        member: String,
        name: String,
        source: std::string::FromUtf8Error,
    },
    NativeInNoneAny {
        name: String,
        path: String,
    },
    BuildOnlyFile {
        name: String,
        path: String,
    },
    TagNotListed {
        name: String,
        tag: String,
    },
    PurelibWithPlatform(String),
    WrongPlatform {
        name: String,
        platforms: String,
        path: String,
    },
    WrongAbi {
        name: String,
        abi: String,
        path: String,
        built: String,
    },
    PairMismatch(Box<PairMismatch>),
    Wheelhouse(PathIoError),
    Copy {
        path: PathBuf,
        source: io::Error,
    },
    NoTrustedPublishing(String),
    HttpClient(dx_http::Error),
    IdentityRequest(dx_http::Error),
    IdentityNoValue,
    CrossRepo {
        workflow_ref: String,
        repository: String,
    },
    Exchange(dx_http::Error),
    ExchangeRefused {
        status: u16,
        detail: String,
    },
    MissingCredentials,
    SignRun(RunError),
    SignFailed(String),
    NoAttestation(PathBuf),
    InvalidBaseline(String),
    TempDir(io::Error),
    Griffe(RunError),
    ApiCheck(ApiCheckError),
}

impl fmt::Display for PypiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWheelName(base) => write!(
                f,
                "invalid wheel filename {base:?}: want name-version-python-abi-platform.whl"
            ),
            Self::InvalidSdistName(base) => {
                write!(
                    f,
                    "invalid sdist filename {base:?}: want name-version.tar.gz"
                )
            }
            Self::UnsupportedArtifact(base) => {
                write!(f, "unsupported artifact {base:?}: want a .whl or .tar.gz")
            }
            Self::InvalidIdentity(base) => {
                write!(f, "invalid distribution name or version in {base:?}")
            }
            Self::Basename(source) => write!(f, "{source}"),
            Self::OpenWheel { name, source } => write!(f, "cannot open wheel {name}: {source}"),
            Self::NotZip { name, source } => {
                write!(f, "wheel {name} is not a zip file: {source}")
            }
            Self::NoRecord(name) => write!(f, "wheel {name} has no .dist-info/RECORD"),
            Self::ReadMember {
                member,
                name,
                source,
            } => write!(f, "cannot read {member} in wheel {name}: {source}"),
            Self::MissingMember { member, name } => {
                write!(f, "cannot read {member} in wheel {name}: no such member")
            }
            Self::MemberUtf8 {
                member,
                name,
                source,
            } => write!(f, "cannot read {member} in wheel {name}: {source}"),
            Self::NativeInNoneAny { name, path } => {
                write!(f, "wheel {name} is tagged none-any but contains {path}")
            }
            Self::BuildOnlyFile { name, path } => {
                write!(f, "wheel {name} ships build-only file {path}")
            }
            Self::TagNotListed { name, tag } => write!(
                f,
                "wheel {name} is named {tag} but its WHEEL file does not list that tag"
            ),
            Self::PurelibWithPlatform(name) => write!(
                f,
                "wheel {name} has a platform tag but its WHEEL file says Root-Is-Purelib: true"
            ),
            Self::WrongPlatform {
                name,
                platforms,
                path,
            } => write!(f, "wheel {name} is tagged {platforms} but contains {path}"),
            Self::WrongAbi {
                name,
                abi,
                path,
                built,
            } => write!(
                f,
                "wheel {name} is tagged {abi} but contains {path} built for {built}"
            ),
            Self::PairMismatch(pair) => write!(
                f,
                "wheel {} is {} {} but sdist {} is {} {}",
                pair.wheel,
                pair.project,
                pair.version,
                pair.sdist,
                pair.sdist_project,
                pair.sdist_version
            ),
            Self::Wheelhouse(source) => write!(f, "{source}"),
            Self::Copy { path, source } => {
                write!(f, "cannot stage {}: {source}", path.display())
            }
            Self::NoTrustedPublishing(url) => write!(
                f,
                "trusted publishing is not available for {url}; set PYPI_API_TOKEN"
            ),
            Self::HttpClient(source) => write!(f, "{source}"),
            Self::IdentityRequest(source) => {
                write!(f, "CI identity token request failed: {source}")
            }
            Self::IdentityNoValue => f.write_str("CI identity token response has no value"),
            Self::CrossRepo {
                workflow_ref,
                repository,
            } => write!(
                f,
                "PyPI trusted publishing does not accept a reusable workflow from another \
                 repository ({workflow_ref}); set PYPI_API_TOKEN or publish from a workflow in {repository}"
            ),
            Self::Exchange(source) => write!(f, "token exchange failed: {source}"),
            Self::ExchangeRefused { status, detail } => {
                write!(f, "token exchange failed with HTTP {status}: {detail}")
            }
            Self::MissingCredentials => f.write_str(
                "live upload needs PYPI_API_TOKEN or a trusted-publisher identity \
                 (ACTIONS_ID_TOKEN_REQUEST_URL and ACTIONS_ID_TOKEN_REQUEST_TOKEN); refusing",
            ),
            Self::SignRun(source) => write!(f, "{source}"),
            Self::SignFailed(exit) => write!(f, "pypi-attestations sign failed with {exit}"),
            Self::NoAttestation(path) => {
                write!(f, "pypi-attestations wrote no {}", path.display())
            }
            Self::InvalidBaseline(baseline) => write!(f, "invalid api_baseline '{baseline}'"),
            Self::TempDir(source) => write!(f, "{source}"),
            Self::Griffe(source) => write!(f, "griffe: {source}"),
            Self::ApiCheck(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for PypiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Basename(source)
            | Self::OpenWheel { source, .. }
            | Self::NotZip { source, .. }
            | Self::ReadMember { source, .. }
            | Self::Copy { source, .. }
            | Self::TempDir(source) => Some(source),
            Self::MemberUtf8 { source, .. } => Some(source),
            Self::Wheelhouse(source) => Some(source),
            Self::HttpClient(source) | Self::IdentityRequest(source) | Self::Exchange(source) => {
                Some(source)
            }
            Self::SignRun(source) | Self::Griffe(source) => Some(source),
            Self::ApiCheck(source) => Some(source),
            Self::InvalidWheelName(_)
            | Self::InvalidSdistName(_)
            | Self::UnsupportedArtifact(_)
            | Self::InvalidIdentity(_)
            | Self::NoRecord(_)
            | Self::MissingMember { .. }
            | Self::NativeInNoneAny { .. }
            | Self::BuildOnlyFile { .. }
            | Self::TagNotListed { .. }
            | Self::PurelibWithPlatform(_)
            | Self::WrongPlatform { .. }
            | Self::WrongAbi { .. }
            | Self::PairMismatch(_)
            | Self::NoTrustedPublishing(_)
            | Self::IdentityNoValue
            | Self::CrossRepo { .. }
            | Self::ExchangeRefused { .. }
            | Self::MissingCredentials
            | Self::SignFailed(_)
            | Self::NoAttestation(_)
            | Self::InvalidBaseline(_) => None,
        }
    }
}

fn pypi_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub fn pypi_artifact_identity(base: &str) -> Result<(String, String), PypiError> {
    let (name, version) = if let Some(stem) = base.strip_suffix(".whl") {
        let parts: Vec<&str> = stem.split('-').collect();
        match parts.as_slice() {
            &[name, version, _, _, _] | &[name, version, _, _, _, _] => (name, version),
            _ => return Err(PypiError::InvalidWheelName(base.to_owned())),
        }
    } else if let Some(stem) = base.strip_suffix(".tar.gz") {
        stem.rsplit_once('-')
            .ok_or_else(|| PypiError::InvalidSdistName(base.to_owned()))?
    } else {
        return Err(PypiError::UnsupportedArtifact(base.to_owned()));
    };
    if !pypi_valid_name(name) || version.is_empty() {
        return Err(PypiError::InvalidIdentity(base.to_owned()));
    }
    Ok((dx_ecosystems::pep503_name(name), version.to_owned()))
}

const NATIVE_SUFFIXES: &[&str] = &[".so", ".pyd", ".dylib"];
const BUILD_ONLY_SUFFIXES: &[&str] = &[".h", ".a", ".lib", ".obj", ".o", ".pdb"];

fn pypi_check_wheel_contents(wheel: &Path) -> Result<(), PypiError> {
    let name = basename(wheel).map_err(PypiError::Basename)?;
    let file = std::fs::File::open(wheel).map_err(|source| PypiError::OpenWheel {
        name: name.clone(),
        source,
    })?;
    let names = dx_archive::zip_names(file).map_err(|source| PypiError::NotZip {
        name: name.clone(),
        source,
    })?;
    let record_name = names
        .into_iter()
        .find(|entry| entry.ends_with(".dist-info/RECORD") && entry.matches('/').count() == 1)
        .ok_or_else(|| PypiError::NoRecord(name.clone()))?;
    let record = read_wheel_text(wheel, &name, &record_name)?;
    let none_any = name.ends_with("-none-any.whl");
    let tags = FilenameTags::parse(&name);
    let mut has_native = false;
    for line in record.lines() {
        let path = line.split(',').next().unwrap_or("");
        let lower = path.to_ascii_lowercase();
        let native = NATIVE_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix));
        has_native |= native;
        if native && let Some(tags) = &tags {
            tags.check_native(&name, path, &lower)?;
        }
        if none_any && NATIVE_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix)) {
            return Err(PypiError::NativeInNoneAny {
                name,
                path: path.to_owned(),
            });
        }
        if BUILD_ONLY_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        {
            return Err(PypiError::BuildOnlyFile {
                name,
                path: path.to_owned(),
            });
        }
    }
    if let Some(tags) = tags
        && (has_native || !tags.platforms.iter().all(|platform| platform == "any"))
    {
        let wheel_name = record_name.replace("RECORD", "WHEEL");
        let metadata = read_wheel_text(wheel, &name, &wheel_name)?;
        tags.check_metadata(&name, &metadata)?;
    }
    Ok(())
}

fn read_wheel_text(wheel: &Path, name: &str, member: &str) -> Result<String, PypiError> {
    let bytes = dx_archive::read_zip_member(wheel, member, 64 << 20)
        .map_err(|source| PypiError::ReadMember {
            member: member.to_owned(),
            name: name.to_owned(),
            source,
        })?
        .ok_or_else(|| PypiError::MissingMember {
            member: member.to_owned(),
            name: name.to_owned(),
        })?;
    String::from_utf8(bytes).map_err(|source| PypiError::MemberUtf8 {
        member: member.to_owned(),
        name: name.to_owned(),
        source,
    })
}

struct FilenameTags {
    pythons: Vec<String>,
    abis: Vec<String>,
    platforms: Vec<String>,
}

impl FilenameTags {
    fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".whl")?;
        let mut fields = stem.rsplit('-');
        let platform = fields.next()?;
        let abi = fields.next()?;
        let python = fields.next()?;
        let split = |field: &str| field.split('.').map(str::to_owned).collect();
        Some(Self {
            pythons: split(python),
            abis: split(abi),
            platforms: split(platform),
        })
    }

    fn abi_label(&self) -> String {
        self.abis.join(".")
    }

    fn check_metadata(&self, name: &str, metadata: &str) -> Result<(), PypiError> {
        let declared: Vec<&str> = metadata
            .lines()
            .filter_map(|line| line.strip_prefix("Tag:"))
            .map(str::trim)
            .collect();
        for python in &self.pythons {
            for abi in &self.abis {
                for platform in &self.platforms {
                    let tag = format!("{python}-{abi}-{platform}");
                    if !declared.contains(&tag.as_str()) {
                        return Err(PypiError::TagNotListed {
                            name: name.to_owned(),
                            tag,
                        });
                    }
                }
            }
        }
        let platform_any = self.platforms.iter().all(|platform| platform == "any");
        let purelib = metadata
            .lines()
            .filter_map(|line| line.strip_prefix("Root-Is-Purelib:"))
            .any(|value| value.trim().eq_ignore_ascii_case("true"));
        if !platform_any && purelib {
            return Err(PypiError::PurelibWithPlatform(name.to_owned()));
        }
        Ok(())
    }

    fn check_native(&self, name: &str, path: &str, lower: &str) -> Result<(), PypiError> {
        let abi = self.abi_label();
        let on = |needle: &str| {
            self.platforms
                .iter()
                .any(|platform| platform.contains(needle))
        };
        let wrong_platform = || PypiError::WrongPlatform {
            name: name.to_owned(),
            platforms: self.platforms.join("."),
            path: path.to_owned(),
        };
        if lower.ends_with(".pyd") && on("linux") {
            return Err(wrong_platform());
        }
        if lower.ends_with(".so") && self.platforms.iter().any(|p| p.starts_with("win")) {
            return Err(wrong_platform());
        }
        let wrong_abi = |built: String| PypiError::WrongAbi {
            name: name.to_owned(),
            abi: abi.clone(),
            path: path.to_owned(),
            built,
        };
        if let Some(built) = path_python_tag(lower) {
            if !self.abis.contains(&built) {
                return Err(wrong_abi(built));
            }
        } else if lower.contains(".abi3.") && !self.abis.iter().any(|tag| tag == "abi3") {
            return Err(wrong_abi("abi3".to_owned()));
        }
        Ok(())
    }
}

fn path_python_tag(lower: &str) -> Option<String> {
    let file = lower.rsplit('/').next()?;
    for marker in [".cpython-3", ".cp3"] {
        if let Some((_, rest)) = file.split_once(marker) {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty()
                && rest
                    .get(digits.len()..)
                    .is_some_and(|tail| tail.starts_with('-'))
            {
                return Some(format!("cp3{digits}"));
            }
        }
    }
    None
}

fn pypi_check_pair(
    wheel_src: &Path,
    sdist_src: Option<&Path>,
) -> Result<(String, String), PypiError> {
    let wheel = basename(wheel_src).map_err(PypiError::Basename)?;
    let (project, version) = pypi_artifact_identity(&wheel)?;
    pypi_check_wheel_contents(wheel_src)?;
    if let Some(sdist_src) = sdist_src {
        let sdist = basename(sdist_src).map_err(PypiError::Basename)?;
        let (sdist_project, sdist_version) = pypi_artifact_identity(&sdist)?;
        if sdist_project != project || sdist_version != version {
            return Err(PypiError::PairMismatch(Box::new(PairMismatch {
                wheel,
                project,
                version,
                sdist,
                sdist_project,
                sdist_version,
            })));
        }
    }
    Ok((project, version))
}

pub fn pypi_build_wheelhouse(
    wheel_src: &Path,
    sdist_src: Option<&Path>,
    outdir: &Path,
    dist_name: &str,
) -> Result<PathBuf, PypiError> {
    let (project, _) = pypi_check_pair(wheel_src, sdist_src)?;
    let house = outdir.join(format!("{dist_name}-wheelhouse"));
    path_io::create_dir_all(&house).map_err(PypiError::Wheelhouse)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut sources: Vec<&Path> = vec![wheel_src];
    if let Some(sdist) = sdist_src {
        sources.push(sdist);
    }
    for src in sources {
        let base = basename(src).map_err(PypiError::Basename)?;
        let dest = house.join(&base);
        let digest = copy_verified(src, &dest).map_err(|source| PypiError::Copy {
            path: src.to_path_buf(),
            source,
        })?;
        entries.push((base, digest));
    }
    entries.sort();
    let index_dir = house.join("simple-index").join(&project);
    path_io::create_dir_all(&index_dir).map_err(PypiError::Wheelhouse)?;
    let mut body = String::from("<!DOCTYPE html>\n<html><body>\n");
    for (base, digest) in &entries {
        body.push_str(&format!(
            "<a href=\"../../{base}#sha256={digest}\">{base}</a><br/>\n"
        ));
    }
    body.push_str("</body></html>\n");
    path_io::write_scratch(&index_dir.join("index.html"), body.as_bytes())
        .map_err(PypiError::Wheelhouse)?;
    Ok(house)
}

pub(crate) const PYPI_GATE: OwnerGate = OwnerGate {
    tool: "pypi_deploy",
    action: "upload",
    credentials: &[],
    approved: "PYPI_PUBLISH_APPROVED",
};

const PYPI_OIDC_ENV: &[&str] = &[
    "ACTIONS_ID_TOKEN_REQUEST_URL",
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "SIGSTORE_ID_TOKEN",
    "REQUESTS_CA_BUNDLE",
    "SSL_CERT_FILE",
];

fn pypi_oidc_present(var: impl Fn(&str) -> String) -> bool {
    !var("ACTIONS_ID_TOKEN_REQUEST_URL").is_empty()
        && !var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").is_empty()
}

#[derive(Debug, PartialEq)]
enum PypiAuth {
    Token(String),
    TrustedPublishing,
    Missing,
}

fn pypi_auth(var: impl Fn(&str) -> String) -> PypiAuth {
    let token = var("PYPI_API_TOKEN");
    if !token.is_empty() {
        PypiAuth::Token(token)
    } else if pypi_oidc_present(&var) {
        PypiAuth::TrustedPublishing
    } else {
        PypiAuth::Missing
    }
}

fn pypi_mint_endpoint(registry_url: &str) -> Option<(&'static str, String)> {
    let rest = registry_url.strip_prefix("https://")?;
    let host = rest.split('/').next()?;
    match host {
        "upload.pypi.org" => Some(("pypi", "https://pypi.org/_/oidc/mint-token".to_owned())),
        "test.pypi.org" => Some((
            "testpypi",
            "https://test.pypi.org/_/oidc/mint-token".to_owned(),
        )),
        _ => None,
    }
}

fn pypi_cross_repo_refusal(jwt: &str) -> Option<PypiError> {
    use base64::Engine as _;
    let payload = jwt.split('.').nth(1)?.trim_end_matches('=');
    let claims: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .ok()?,
    )
    .ok()?;
    let workflow_ref = claims.get("job_workflow_ref")?.as_str()?;
    let repository = claims.get("repository")?.as_str()?;
    let (workflow_repo, _) = workflow_ref.split_once("/.github/workflows/")?;
    (!workflow_repo.eq_ignore_ascii_case(repository)).then(|| PypiError::CrossRepo {
        workflow_ref: workflow_ref.to_owned(),
        repository: repository.to_owned(),
    })
}

fn pypi_mint_errors(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return String::from_utf8_lossy(body).trim().to_owned();
    };
    value
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .map(|error| {
                    let field = |key| error.get(key).and_then(serde_json::Value::as_str);
                    format!(
                        "{}: {}",
                        field("code").unwrap_or("?"),
                        field("description").unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default()
}

fn pypi_mint_token(
    client: &dx_http::Client,
    registry_url: &str,
    var: impl Fn(&str) -> String,
) -> Result<String, PypiError> {
    let (audience, mint) = pypi_mint_endpoint(registry_url)
        .ok_or_else(|| PypiError::NoTrustedPublishing(registry_url.to_owned()))?;
    let mut url = var("ACTIONS_ID_TOKEN_REQUEST_URL");
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str("audience=");
    url.push_str(&crate::http::query_escape(audience));
    let authorization = format!("bearer {}", var("ACTIONS_ID_TOKEN_REQUEST_TOKEN"));
    let identity = client
        .text(&url, &[("Authorization", authorization.as_str())], 64 << 10)
        .map_err(PypiError::IdentityRequest)?;
    let jwt = serde_json::from_str::<serde_json::Value>(&identity)
        .ok()
        .and_then(|value| value.get("value")?.as_str().map(str::to_owned))
        .ok_or(PypiError::IdentityNoValue)?;
    if let Some(refusal) = pypi_cross_repo_refusal(&jwt) {
        return Err(refusal);
    }
    let body = serde_json::json!({ "token": jwt }).to_string();
    let reply = client
        .send(
            dx_http::Method::Post,
            &mint,
            &[("Content-Type", "application/json")],
            dx_http::Body::Bytes(body.as_bytes()),
            64 << 10,
        )
        .map_err(PypiError::Exchange)?;
    let token = serde_json::from_slice::<serde_json::Value>(&reply.body)
        .ok()
        .and_then(|value| value.get("token")?.as_str().map(str::to_owned))
        .filter(|token| !token.is_empty());
    if let (true, Some(token)) = ((200..300).contains(&reply.status), token) {
        return Ok(token);
    }
    Err(PypiError::ExchangeRefused {
        status: reply.status,
        detail: pypi_scrub(pypi_mint_errors(&reply.body).as_bytes(), &jwt),
    })
}

fn pypi_credentials(
    auth: PypiAuth,
    mint: impl FnOnce() -> Result<String, PypiError>,
) -> Result<(String, bool), PypiError> {
    match auth {
        PypiAuth::Token(token) => Ok((token, false)),
        PypiAuth::TrustedPublishing => mint().map(|token| (token, true)),
        PypiAuth::Missing => Err(PypiError::MissingCredentials),
    }
}

fn pypi_sign(signer: &Path, files: &[std::path::PathBuf]) -> Result<Vec<PathBuf>, PypiError> {
    let mut cmd = scrubbed_command(signer, &[PUBLISH_CHILD_ENV_KEEP, PYPI_OIDC_ENV], &[]);
    cmd.arg("sign").args(files);
    let out = run_publish(&mut cmd).map_err(PypiError::SignRun)?;
    if !out.success() {
        eprint!("{}", pypi_scrub(&out.stderr, ""));
        return Err(PypiError::SignFailed(exit_text(&out)));
    }
    let attestations: Vec<std::path::PathBuf> = files
        .iter()
        .map(|file| {
            let mut name = file.as_os_str().to_owned();
            name.push(".publish.attestation");
            std::path::PathBuf::from(name)
        })
        .collect();
    match attestations.iter().find(|path| !path.is_file()) {
        Some(missing) => Err(PypiError::NoAttestation(missing.clone())),
        None => Ok(attestations),
    }
}

const TWINE_CHILD_ENV_EXTRA: &[&str] = &["REQUESTS_CA_BUNDLE"];

fn pypi_scrub(text: &[u8], token: &str) -> String {
    crate::run::pages_redact(&String::from_utf8_lossy(text), token)
}

fn pypi_duplicate_note(output: &str, base: &str, version: &str, url: &str) -> Option<String> {
    output
        .to_ascii_lowercase()
        .contains("already exists")
        .then(|| {
            format!(
                "pypi_deploy: {base} version {version} is already on {url}; upload a new version"
            )
        })
}

fn pypi_upload(
    twine: &Path,
    wheel: &Path,
    sdist: Option<&Path>,
    stage: &Path,
    registry_url: &str,
    token: &str,
    signer: Option<&Path>,
) -> i32 {
    let (_, version) = match pypi_check_pair(wheel, sdist) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    };
    let code = pypi_upload_staged(
        twine,
        (wheel, sdist),
        stage,
        (registry_url, token, signer),
        &version,
    );
    dx_atomic_fs::discard_dir(stage);
    code
}

fn pypi_upload_staged(
    twine: &Path,
    (wheel, sdist): (&Path, Option<&Path>),
    stage: &Path,
    (registry_url, token, signer): (&str, &str, Option<&Path>),
    version: &str,
) -> i32 {
    let mut files = Vec::new();
    for src in std::iter::once(wheel).chain(sdist) {
        let copied = basename(src).and_then(|base| {
            std::fs::create_dir_all(stage)?;
            let dest = stage.join(base);
            copy_verified(src, &dest)?;
            Ok(dest)
        });
        match copied {
            Ok(dest) => files.push(dest),
            Err(error) => {
                eprintln!("pypi_deploy: {error}");
                return 1;
            }
        }
    }
    let command = |verb: &str| {
        let mut cmd = scrubbed_command(
            twine,
            &[PUBLISH_CHILD_ENV_KEEP, TWINE_CHILD_ENV_EXTRA],
            &[("TWINE_USERNAME", "__token__"), ("TWINE_PASSWORD", token)],
        );
        cmd.arg(verb);
        cmd
    };
    let mut check = command("check");
    check.arg("--strict").args(&files);
    match run_publish(&mut check) {
        Ok(out) if out.success() => {}
        Ok(out) => {
            eprint!("{}", pypi_scrub(&out.stdout, token));
            eprint!("{}", pypi_scrub(&out.stderr, token));
            eprintln!("pypi_deploy: twine check failed with {}", exit_text(&out));
            return 1;
        }
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    }
    let attestations = match signer.map(|signer| pypi_sign(signer, &files)) {
        Some(Ok(paths)) => paths,
        Some(Err(error)) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
        None => Vec::new(),
    };
    let mut upload = command("upload");
    upload
        .arg("--non-interactive")
        .arg("--repository-url")
        .arg(registry_url)
        .args(&files);
    if !attestations.is_empty() {
        upload.arg("--attestations").args(&attestations);
    }
    let out = match run_publish(&mut upload) {
        Ok(out) => out,
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    };
    let stdout = pypi_scrub(&out.stdout, token);
    let stderr = pypi_scrub(&out.stderr, token);
    print!("{stdout}");
    eprint!("{stderr}");
    let base = wheel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if out.success() {
        println!("pypi_deploy: uploaded {base} to {registry_url}");
        return 0;
    }
    if let Some(note) =
        pypi_duplicate_note(&format!("{stdout}{stderr}"), base, version, registry_url)
    {
        eprintln!("{note}");
    }
    eprintln!("pypi_deploy: twine failed with {}", exit_text(&out));
    1
}

pub struct PypiApiCheck<'a> {
    pub mode: &'a str,
    pub baseline: &'a str,
    pub package: &'a str,
    pub src: &'a str,
    pub griffe_rloc: &'a str,
    pub attestations_rloc: &'a str,
}

fn griffe_changes(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| {
            line.split_once(".py:")
                .is_some_and(|(_, rest)| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .map(str::to_owned)
        .collect()
}

fn pypi_previous_wheel(body: &str, next: &str) -> Option<(String, Option<String>)> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let releases = json.get("releases")?.as_object()?;
    let previous = highest_below(releases.keys().map(String::as_str), next)?;
    let wheel = releases[&previous]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|file| file["packagetype"] == "bdist_wheel" && file["yanked"] != true)
        .find_map(|file| file["url"].as_str().map(str::to_owned));
    Some((previous, wheel))
}

fn pypi_registry_baseline(
    project: &str,
    version: &str,
    wheel: &Path,
    work: &Path,
) -> Result<(String, Vec<String>), Outcome> {
    let url = format!("https://pypi.org/pypi/{project}/json");
    let body = match fetch(&url, 64 << 20) {
        Ok(Some(body)) => body,
        Ok(None) => return Err(Outcome::FirstRelease),
        Err(error) => return Err(Outcome::Skipped(error.to_string())),
    };
    let Some((previous, wheel_url)) = pypi_previous_wheel(&String::from_utf8_lossy(&body), version)
    else {
        return Err(Outcome::FirstRelease);
    };
    let Some(wheel_url) = wheel_url else {
        return Err(Outcome::Skipped(format!(
            "{project} {previous} has no wheel"
        )));
    };
    match fetch_to(&wheel_url, &work.join("old.zip"), 512 << 20) {
        Ok(true) => {}
        Ok(false) => return Err(Outcome::Skipped(format!("{wheel_url} not found"))),
        Err(error) => return Err(Outcome::Skipped(error.to_string())),
    }
    let extracted = std::fs::copy(wheel, work.join("new.zip"))
        .map(drop)
        .and_then(|()| archive_extract(&work.join("old.zip"), &work.join("old")).map(drop))
        .and_then(|()| archive_extract(&work.join("new.zip"), &work.join("new")).map(drop));
    if let Err(error) = extracted {
        return Err(Outcome::Skipped(format!(
            "cannot extract the wheels: {error}"
        )));
    }
    let args = vec![
        "--old-search".to_owned(),
        work.join("old").display().to_string(),
        "--search".to_owned(),
        work.join("new").display().to_string(),
    ];
    Ok((previous, args))
}

fn pypi_api_check(
    griffe: &Path,
    (project, version): (&str, &str),
    wheel: &Path,
    check: &PypiApiCheck,
    workspace: &Path,
) -> Result<Outcome, PypiError> {
    let baseline = Baseline::parse(check.baseline)
        .ok_or_else(|| PypiError::InvalidBaseline(check.baseline.to_owned()))?;
    let work = tempfile_stage_dir("pypi-api-").map_err(PypiError::TempDir)?;
    let found = if baseline == Baseline::Git {
        match previous_git_tag(workspace, version) {
            Ok(Some(tag)) => Ok((
                tag.trim_start_matches('v').to_owned(),
                vec![
                    "--against".to_owned(),
                    tag,
                    "--search".to_owned(),
                    check.src.to_owned(),
                ],
            )),
            Ok(None) => Err(Outcome::FirstRelease),
            Err(error) => Err(Outcome::Skipped(error.to_string())),
        }
    } else {
        pypi_registry_baseline(project, version, wheel, &work)
    };
    let outcome = match found {
        Ok((previous, args)) => {
            let mut cmd = scrubbed_command(griffe, &[PUBLISH_CHILD_ENV_KEEP], &[]);
            cmd.args(["check", check.package])
                .args(&args)
                .current_dir(workspace);
            run_publish(&mut cmd)
                .map_err(PypiError::Griffe)
                .map(|output| {
                    let text = format!(
                        "{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let success = output.success();
                    let changes = if success {
                        Vec::new()
                    } else {
                        griffe_changes(&text)
                    };
                    Outcome::Compared {
                        previous,
                        changes: changes_or_failure(changes, success, "griffe", &text),
                    }
                })
        }
        Err(outcome) => Ok(outcome),
    };
    dx_atomic_fs::discard_dir(&work);
    outcome
}

pub fn pypi_main(
    wheel_rloc: &str,
    sdist_rloc: &str,
    dist_name: &str,
    registry_url: &str,
    argv: &[String],
) -> i32 {
    let off = PypiApiCheck {
        mode: "off",
        baseline: "registry",
        package: "",
        src: "",
        griffe_rloc: "",
        attestations_rloc: "",
    };
    pypi_main_checked(wheel_rloc, sdist_rloc, dist_name, registry_url, &off, argv)
}

pub fn pypi_main_checked(
    wheel_rloc: &str,
    sdist_rloc: &str,
    dist_name: &str,
    registry_url: &str,
    api: &PypiApiCheck,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "pypi_deploy",
        "the wheelhouse is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(wheel) = run.resolve(wheel_rloc) else {
        return 1;
    };
    let Some(sdist) = run.resolve_optional(sdist_rloc) else {
        return 1;
    };
    let Some(mode) = ApiCheckMode::parse(api.mode) else {
        eprintln!("pypi_deploy: invalid api_compat '{}'", api.mode);
        return 1;
    };
    let checked = pypi_check_pair(&wheel, sdist.as_deref()).and_then(|(project, version)| {
        if mode == ApiCheckMode::Off {
            return Ok(());
        }
        let griffe = if api.griffe_rloc.is_empty() {
            None
        } else {
            run.resolve(api.griffe_rloc)
        };
        let outcome = match griffe {
            Some(griffe) => {
                pypi_api_check(&griffe, (&project, &version), &wheel, api, &workspace_dir())?
            }
            None => Outcome::Skipped("griffe is not available".to_owned()),
        };
        conclude("pypi_deploy", mode, &version, outcome).map_err(PypiError::ApiCheck)
    });
    match checked {
        Ok(()) => {}
        Err(PypiError::ApiCheck(error)) => {
            eprintln!("{error}");
            return 1;
        }
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            return 1;
        }
    }
    if OwnerGate::requested(env_or_empty) {
        if let Err(error) = PYPI_GATE.authorize(env_or_empty) {
            eprintln!("{error}");
            return 1;
        }
        let credentials = pypi_credentials(pypi_auth(env_or_empty), || {
            crate::http::client()
                .map_err(PypiError::HttpClient)
                .and_then(|client| pypi_mint_token(&client, registry_url, env_or_empty))
        });
        let (token, attest) = match credentials {
            Ok(credentials) => credentials,
            Err(error) => {
                eprintln!("pypi_deploy: {error}");
                return 1;
            }
        };
        let signer = if !attest {
            None
        } else if api.attestations_rloc.is_empty() {
            eprintln!(
                "pypi_deploy: attestation signing needs the pinned pypi-attestations; refusing"
            );
            return 1;
        } else {
            match run.resolve(api.attestations_rloc) {
                Some(signer) => Some(signer),
                None => return 1,
            }
        };
        return pypi_upload(
            Path::new("twine"),
            &wheel,
            sdist.as_deref(),
            &outdir.join(format!("{dist_name}-upload")),
            registry_url,
            &token,
            signer.as_deref(),
        );
    }
    match pypi_build_wheelhouse(&wheel, sdist.as_deref(), outdir, dist_name) {
        Ok(house) => {
            let project = basename(&wheel)
                .map_err(PypiError::Basename)
                .and_then(|base| pypi_artifact_identity(&base))
                .map(|(project, _)| project)
                .unwrap_or_default();
            let build_mode = deploy_build_mode();
            println!("pypi_deploy: staged {dist_name} (build_mode: {build_mode})");
            let auth = match pypi_auth(env_or_empty) {
                PypiAuth::Token(_) => "PYPI_API_TOKEN",
                PypiAuth::TrustedPublishing => "trusted publishing",
                PypiAuth::Missing => "none",
            };
            println!("  auth: {auth}");
            println!("  wheelhouse: {}", house.display());
            println!(
                "  index: {}",
                house
                    .join("simple-index")
                    .join(&project)
                    .join("index.html")
                    .display()
            );
            0
        }
        Err(error) => {
            eprintln!("pypi_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[cfg(unix)]
    #[test]
    fn api_check_with_a_git_baseline_runs_griffe_against_the_tag() {
        if env_test_in_child(
            "pypi::tests::api_check_with_a_git_baseline_runs_griffe_against_the_tag",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let repo = scratch.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        write_crate_file(&repo, "src/pkg/__init__.py", "def hello(): ...\n");
        with_env(&[], || {
            for args in [
                &["init", "-q"][..],
                &["add", "."],
                &["commit", "-q", "-m", "x"],
                &["tag", "v1.2.0"],
            ] {
                dx_test_scratch::git(&repo, &repo, args).unwrap();
            }
        });
        let args_log = scratch.path().join("args");
        let fake = scratch.path().join("griffe");
        dx_testing::executable_script(
            &fake,
            &format!(
                "#!/bin/sh\necho \"$@\" > {}\necho 'src/pkg/__init__.py:1: pkg.hello: Public object was removed'\nexit 1\n",
                args_log.display()
            ),
        );
        let check = PypiApiCheck {
            mode: "fail",
            baseline: "git",
            package: "pkg",
            src: "src",
            griffe_rloc: "",
            attestations_rloc: "",
        };
        let run = |version: &str| {
            with_env(&[], || {
                pypi_api_check(
                    &fake,
                    ("pkg", version),
                    Path::new("unused.whl"),
                    &check,
                    &repo,
                )
            })
        };
        let outcome = run("1.3.0").expect("ran");
        assert_eq!(
            outcome,
            Outcome::Compared {
                previous: "1.2.0".to_owned(),
                changes: vec![
                    "src/pkg/__init__.py:1: pkg.hello: Public object was removed".to_owned()
                ],
            }
        );
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert_eq!(logged.trim(), "check pkg --against v1.2.0 --search src");
        assert_eq!(run("1.0.0").expect("ran"), Outcome::FirstRelease);
    }

    #[test]
    fn griffe_output_counts_one_breaking_change_per_line() {
        let text = "src/a.py:3: a.f: Parameter was removed\nsrc/b.py:9: b.g: Public object was removed\nnote: x\n";
        assert_eq!(griffe_changes(text).len(), 2);
    }

    #[test]
    fn pypi_previous_wheel_picks_the_release_below_and_its_wheel() {
        let body = r#"{"releases":{
            "1.0.0":[{"packagetype":"sdist","url":"https://f/p-1.0.0.tar.gz"},{"packagetype":"bdist_wheel","url":"https://f/p-1.0.0-py3-none-any.whl"}],
            "1.1.0":[{"packagetype":"sdist","url":"https://f/p-1.1.0.tar.gz"}],
            "2.0.0rc1":[],
            "2.0.0":[]}}"#;
        assert_eq!(
            pypi_previous_wheel(body, "1.1.0"),
            Some((
                "1.0.0".to_owned(),
                Some("https://f/p-1.0.0-py3-none-any.whl".to_owned())
            ))
        );
        assert_eq!(
            pypi_previous_wheel(body, "1.2.0"),
            Some(("1.1.0".to_owned(), None))
        );
        assert_eq!(pypi_previous_wheel(body, "1.0.0"), None);
        assert_eq!(pypi_previous_wheel("not json", "1.0.0"), None);
    }

    fn wheel_bytes(files: &[&str]) -> Vec<u8> {
        wheel_bytes_with(files, None)
    }

    fn wheel_bytes_with(files: &[&str], wheel_file: Option<&str>) -> Vec<u8> {
        let mut record = String::from("pkg-1.0.dist-info/RECORD,,\n");
        for file in files {
            record.push_str(&format!("{file},sha256=x,1\n"));
        }
        let mut members = vec![("pkg-1.0.dist-info/RECORD", record.as_bytes(), 0o644)];
        if let Some(wheel_file) = wheel_file {
            members.push(("pkg-1.0.dist-info/WHEEL", wheel_file.as_bytes(), 0o644));
        }
        zip_archive(&members).expect("zip")
    }

    fn wheel_file(tags: &[&str], purelib: bool) -> String {
        let mut text = format!("Wheel-Version: 1.0\nRoot-Is-Purelib: {purelib}\n");
        for tag in tags {
            text.push_str(&format!("Tag: {tag}\n"));
        }
        text
    }

    fn wheel_error(name: &str, bytes: &[u8]) -> String {
        let scratch = scratch_dir();
        let wheel = scratch.path().join(name);
        std::fs::write(&wheel, bytes).expect("wheel");
        pypi_check_pair(&wheel, None)
            .expect_err("rejected")
            .to_string()
    }

    #[test]
    fn pypi_rejects_native_file_in_none_any_wheel() {
        let bytes = wheel_bytes(&["pkg/_core.so"]);
        assert_eq!(
            wheel_error("pkg-1.0-py3-none-any.whl", &bytes),
            "wheel pkg-1.0-py3-none-any.whl is tagged none-any but contains pkg/_core.so"
        );
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pkg-1.0-cp312-cp312-linux_x86_64.whl");
        let tagged = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        std::fs::write(&wheel, wheel_bytes_with(&["pkg/_core.so"], Some(&tagged))).expect("wheel");
        assert!(pypi_check_pair(&wheel, None).is_ok());
    }

    #[test]
    fn pypi_rejects_tag_missing_from_wheel_file() {
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.so"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp311-cp311-linux_x86_64.whl", &bytes);
        assert!(error.contains("named cp311-cp311-linux_x86_64"), "{error}");
    }

    #[test]
    fn pypi_rejects_native_wheel_marked_purelib() {
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], true);
        let bytes = wheel_bytes_with(&["pkg/_core.so"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp312-cp312-linux_x86_64.whl", &bytes);
        assert!(error.contains("Root-Is-Purelib: true"), "{error}");
    }

    #[test]
    fn pypi_rejects_extension_built_for_other_python() {
        let metadata = wheel_file(&["cp311-cp311-linux_x86_64"], false);
        let bytes = wheel_bytes_with(
            &["pkg/_core.cpython-312-x86_64-linux-gnu.so"],
            Some(&metadata),
        );
        assert_eq!(
            wheel_error("pkg-1.0-cp311-cp311-linux_x86_64.whl", &bytes),
            "wheel pkg-1.0-cp311-cp311-linux_x86_64.whl is tagged cp311 but contains pkg/_core.cpython-312-x86_64-linux-gnu.so built for cp312"
        );
    }

    #[test]
    fn pypi_accepts_windows_pyd_and_rejects_pyd_on_linux() {
        let scratch = scratch_dir();
        let windows = scratch.path().join("pkg-1.0-cp312-cp312-win_amd64.whl");
        let metadata = wheel_file(&["cp312-cp312-win_amd64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.cp312-win_amd64.pyd"], Some(&metadata));
        std::fs::write(&windows, &bytes).expect("wheel");
        assert!(pypi_check_pair(&windows, None).is_ok());
        let metadata = wheel_file(&["cp312-cp312-linux_x86_64"], false);
        let bytes = wheel_bytes_with(&["pkg/_core.pyd"], Some(&metadata));
        let error = wheel_error("pkg-1.0-cp312-cp312-linux_x86_64.whl", &bytes);
        assert!(error.contains("contains pkg/_core.pyd"), "{error}");
    }

    #[test]
    fn pypi_accepts_dotted_filename_tags_covered_by_wheel_file() {
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pkg-1.0-py3-none-any.whl");
        let metadata = wheel_file(&["py2-none-any", "py3-none-any"], false);
        std::fs::write(&wheel, wheel_bytes_with(&[], Some(&metadata))).expect("wheel");
        assert!(pypi_check_pair(&wheel, None).is_ok());
    }

    #[test]
    fn pypi_rejects_build_only_file() {
        let bytes = wheel_bytes(&["pkg/include/pkg.h"]);
        assert_eq!(
            wheel_error("pkg-1.0-py3-none-any.whl", &bytes),
            "wheel pkg-1.0-py3-none-any.whl ships build-only file pkg/include/pkg.h"
        );
    }

    #[test]
    fn pypi_rejects_non_zip_wheel() {
        let error = wheel_error("pkg-1.0-py3-none-any.whl", b"not a zip");
        assert!(error.contains("is not a zip file"), "{error}");
    }

    #[test]
    fn pypi_wheelhouse_builds_index() {
        let scratch = scratch_dir();
        let wheel = scratch.path().join("pypi_demo-0.0.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("write wheel");
        let sdist = scratch.path().join("pypi_demo-0.0.0.tar.gz");
        std::fs::write(&sdist, b"pypi-demo-sdist-bytes-v1").expect("write sdist");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house =
            pypi_build_wheelhouse(&wheel, Some(&sdist), &outdir, "pypi_demo").expect("house");
        assert_eq!(house, outdir.join("pypi_demo-wheelhouse"));
        for src in [&wheel, &sdist] {
            let base = basename(src).expect("base");
            let dest = house.join(&base);
            assert!(dest.is_file());
            assert_eq!(
                dx_digest::sha256_file_hex(&dest).expect("hex"),
                dx_digest::sha256_file_hex(src).expect("hex")
            );
        }
        let index = house
            .join("simple-index")
            .join("pypi-demo")
            .join("index.html");
        let body = std::fs::read_to_string(&index).expect("index");
        assert!(body.contains("pypi_demo-0.0.0-py3-none-any.whl"));
        assert!(body.contains("pypi_demo-0.0.0.tar.gz"));
        assert!(body.contains("#sha256="));
        let house_only =
            pypi_build_wheelhouse(&wheel, None, &outdir, "pypi_demo").expect("wheel only");
        assert!(house_only.join(wheel.file_name().expect("base")).is_file());
    }

    #[test]
    fn pypi_index_uses_normalized_distribution_identity() {
        assert_eq!(
            pypi_artifact_identity("My_Pkg.Name-1.0-py3-none-any.whl").expect("wheel"),
            ("my-pkg-name".to_owned(), "1.0".to_owned())
        );
        assert_eq!(
            pypi_artifact_identity("My_Pkg.Name-1.0.tar.gz").expect("sdist"),
            ("my-pkg-name".to_owned(), "1.0".to_owned())
        );
        assert!(pypi_artifact_identity("broken.whl").is_err());
        assert!(pypi_artifact_identity("pkg.zip").is_err());
        let scratch = scratch_dir();
        let wheel = scratch.path().join("My_Pkg.Name-1.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("wheel");
        let sdist = scratch.path().join("my-pkg-name-1.0.tar.gz");
        std::fs::write(&sdist, b"s").expect("sdist");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house = pypi_build_wheelhouse(&wheel, Some(&sdist), &outdir, "publish").expect("house");
        assert_eq!(house, outdir.join("publish-wheelhouse"));
        assert!(
            house
                .join("simple-index")
                .join("my-pkg-name")
                .join("index.html")
                .is_file()
        );
        assert!(!house.join("simple-index").join("publish").exists());
        let other = scratch.path().join("other-1.0.tar.gz");
        std::fs::write(&other, b"o").expect("other");
        let error =
            pypi_build_wheelhouse(&wheel, Some(&other), &outdir, "publish").expect_err("conflict");
        assert!(error.to_string().contains("my-pkg-name 1.0"), "{error}");
        let newer = scratch.path().join("my_pkg_name-2.0.tar.gz");
        std::fs::write(&newer, b"n").expect("newer");
        assert!(pypi_build_wheelhouse(&wheel, Some(&newer), &outdir, "publish").is_err());
    }

    #[cfg(unix)]
    fn fake_tool(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        dx_testing::executable_script(&path, &format!("#!/bin/sh\n{body}\n"));
        path
    }

    #[cfg(unix)]
    fn fake_twine(dir: &Path, body: &str) -> std::path::PathBuf {
        fake_tool(dir, "twine", body)
    }

    #[cfg(unix)]
    fn upload_fixture(dir: &Path, sdist_name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let wheel = dir.join("pkg-1.0-py3-none-any.whl");
        std::fs::write(&wheel, wheel_bytes(&[])).expect("wheel");
        let sdist = dir.join(sdist_name);
        std::fs::write(&sdist, b"s").expect("sdist");
        (wheel, sdist)
    }

    #[test]
    #[cfg(unix)]
    fn pypi_live_upload_refuses_mismatch_and_uploads_copies() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let argv = dir.join("argv");
        let twine = fake_twine(dir, &format!("echo \"$@\" >> {}", argv.display()));
        let stage = dir.join("stage");
        let (wheel, bad) = upload_fixture(dir, "pkg-2.0.tar.gz");
        assert_eq!(
            pypi_upload(&twine, &wheel, Some(&bad), &stage, "https://u", "tok", None),
            1
        );
        assert!(!argv.exists());
        let good = dir.join("pkg-1.0.tar.gz");
        std::fs::write(&good, b"s").expect("sdist");
        assert_eq!(
            pypi_upload(
                &twine,
                &wheel,
                Some(&good),
                &stage,
                "https://u",
                "tok",
                None
            ),
            0
        );
        let lines = std::fs::read_to_string(&argv).expect("argv");
        let lines: Vec<&str> = lines.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("check --strict "), "{}", lines[0]);
        assert!(
            lines[1].starts_with("upload --non-interactive "),
            "{}",
            lines[1]
        );
        for line in &lines {
            assert!(line.contains(&stage.display().to_string()), "{line}");
            assert!(!line.contains(&wheel.display().to_string()), "{line}");
        }
        assert!(!stage.exists());
    }

    #[test]
    #[cfg(unix)]
    fn pypi_live_upload_stops_on_failed_check() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let marker = dir.join("uploaded");
        let body = format!("[ \"$1\" = check ] && exit 1\ntouch {}", marker.display());
        let twine = fake_twine(dir, &body);
        let (wheel, sdist) = upload_fixture(dir, "pkg-1.0.tar.gz");
        let stage = dir.join("stage");
        let code = pypi_upload(
            &twine,
            &wheel,
            Some(&sdist),
            &stage,
            "https://u",
            "tok",
            None,
        );
        assert_eq!(code, 1);
        assert!(!marker.exists());
    }

    #[test]
    fn pypi_auth_prefers_the_token_variable() {
        fn auth(pairs: &[(&str, &str)]) -> PypiAuth {
            pypi_auth(|key| {
                pairs
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map_or(String::new(), |(_, value)| (*value).to_owned())
            })
        }
        let url = ("ACTIONS_ID_TOKEN_REQUEST_URL", "https://ci/token");
        let request = ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "t");
        assert_eq!(auth(&[]), PypiAuth::Missing);
        assert_eq!(auth(&[url]), PypiAuth::Missing);
        assert_eq!(auth(&[url, request]), PypiAuth::TrustedPublishing);
        assert_eq!(
            auth(&[url, request, ("PYPI_API_TOKEN", "pypi-x")]),
            PypiAuth::Token("pypi-x".to_owned())
        );
    }

    #[test]
    fn pypi_mint_endpoint_names_audience() {
        assert_eq!(
            pypi_mint_endpoint("https://upload.pypi.org/"),
            Some(("pypi", "https://pypi.org/_/oidc/mint-token".to_owned()))
        );
        assert_eq!(
            pypi_mint_endpoint("https://test.pypi.org/").map(|(audience, _)| audience),
            Some("testpypi")
        );
        assert_eq!(pypi_mint_endpoint("https://example.org/"), None);
    }

    #[test]
    #[cfg(unix)]
    fn pypi_signed_upload_passes_attestations() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let argv = dir.join("argv");
        let twine = fake_twine(dir, &format!("echo \"$@\" >> {}", argv.display()));
        let signer = fake_tool(
            dir,
            "pypi_attestations",
            "shift 1\nfor f in \"$@\"; do : > \"$f.publish.attestation\"; done",
        );
        let (wheel, sdist) = upload_fixture(dir, "pkg-1.0.tar.gz");
        let stage = dir.join("stage");
        let code = pypi_upload(
            &twine,
            &wheel,
            Some(&sdist),
            &stage,
            "https://u",
            "tok",
            Some(&signer),
        );
        assert_eq!(code, 0);
        let lines = std::fs::read_to_string(&argv).expect("argv");
        let upload = lines.lines().nth(1).expect("upload line");
        let (files, attestations) = upload.split_once(" --attestations ").expect(upload);
        assert!(!files.contains(".publish.attestation"), "{upload}");
        assert_eq!(attestations.matches(".publish.attestation").count(), 2);
    }

    fn fake_jwt(claims: &serde_json::Value) -> String {
        use base64::Engine as _;
        let part = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        format!(
            "{}.{}.sig",
            part(br#"{"alg":"RS256"}"#),
            part(claims.to_string().as_bytes())
        )
    }

    fn cross_repo_jwt() -> String {
        fake_jwt(&serde_json::json!({
            "repository": "acme/widget",
            "job_workflow_ref": "devhusdk/rules_dx/.github/workflows/reusable-release.yml@refs/tags/x/y",
        }))
    }

    const CROSS_REPO_REFUSAL: &str = "PyPI trusted publishing does not accept a \
        reusable workflow from another repository \
        (devhusdk/rules_dx/.github/workflows/reusable-release.yml@refs/tags/x/y); \
        set PYPI_API_TOKEN or publish from a workflow in acme/widget";

    #[test]
    fn pypi_cross_repo_workflow_is_refused() {
        assert_eq!(
            pypi_cross_repo_refusal(&cross_repo_jwt())
                .map(|refusal| refusal.to_string())
                .as_deref(),
            Some(CROSS_REPO_REFUSAL)
        );
        let same = fake_jwt(&serde_json::json!({
            "repository": "Acme/Widget",
            "job_workflow_ref": "acme/widget/.github/workflows/release.yml@refs/heads/main",
        }));
        assert!(pypi_cross_repo_refusal(&same).is_none());
        assert!(pypi_cross_repo_refusal("junk").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn pypi_mint_refuses_cross_repo_before_exchange() {
        let jwt = cross_repo_jwt();
        let origin = static_registry(|_| {
            vec![(
                "/token?audience=testpypi".to_owned(),
                serde_json::json!({ "value": jwt }).to_string().into_bytes(),
            )]
        });
        let url = format!("{origin}/token");
        let vars = |key: &str| match key {
            "ACTIONS_ID_TOKEN_REQUEST_URL" => url.clone(),
            "ACTIONS_ID_TOKEN_REQUEST_TOKEN" => "request".to_owned(),
            _ => String::new(),
        };
        assert_eq!(
            pypi_mint_token(
                &crate::http::test_client(),
                "https://test.pypi.org/legacy/",
                vars
            )
            .map_err(|error| error.to_string()),
            Err(CROSS_REPO_REFUSAL.to_owned())
        );
    }

    #[test]
    #[cfg(unix)]
    fn pypi_token_upload_is_not_attested() {
        let vars = |key: &str| {
            match key {
                "ACTIONS_ID_TOKEN_REQUEST_URL" => "https://ci/token",
                "ACTIONS_ID_TOKEN_REQUEST_TOKEN" => "t",
                "PYPI_API_TOKEN" => "pypi-x",
                _ => "",
            }
            .to_owned()
        };
        let (token, attest) =
            pypi_credentials(pypi_auth(vars), || panic!("a token upload mints nothing"))
                .expect("credentials");
        assert_eq!((token.as_str(), attest), ("pypi-x", false));
        assert_eq!(
            pypi_credentials(PypiAuth::TrustedPublishing, || Ok("m".to_owned())).expect("minted"),
            ("m".to_owned(), true)
        );
        let scratch = scratch_dir();
        let dir = scratch.path();
        let argv = dir.join("argv");
        let twine = fake_twine(dir, &format!("echo \"$@\" >> {}", argv.display()));
        let (wheel, sdist) = upload_fixture(dir, "pkg-1.0.tar.gz");
        let code = pypi_upload(
            &twine,
            &wheel,
            Some(&sdist),
            &dir.join("stage"),
            "https://u",
            &token,
            None,
        );
        assert_eq!(code, 0);
        let lines = std::fs::read_to_string(&argv).expect("argv");
        let upload = lines.lines().nth(1).expect("upload line");
        assert!(!upload.contains("--attestations"), "{upload}");
    }

    #[test]
    fn pypi_duplicate_version_is_named() {
        let note = pypi_duplicate_note(
            "HTTPError: File Already Exists",
            "pkg.whl",
            "1.0",
            "https://u",
        );
        assert_eq!(
            note.as_deref(),
            Some("pypi_deploy: pkg.whl version 1.0 is already on https://u; upload a new version")
        );
        assert!(pypi_duplicate_note("boom", "pkg.whl", "1.0", "https://u").is_none());
        assert_eq!(pypi_scrub(b"a tok b", "tok"), "a [redacted] b");
    }
}
