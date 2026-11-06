//! npm publisher: deterministic pack, pack verification and the npm publish.

use crate::api_compat::{ApiCompatMode, breaking_refusal, required_bump};
use crate::archive::*;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_profile, env_or_empty, print_redacted,
    scrubbed_command,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Maps npm input files to package-relative paths under the package.json directory.
pub fn npm_package_paths(
    srcs: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    package_relative_paths(srcs, "package.json", "npm pack")
}

/// Creates one deterministic npm pack.
#[allow(clippy::too_many_arguments)]
pub fn npm_create_pack(
    package: &str,
    tag: &str,
    registry: &str,
    access: &str,
    provenance: &str,
    tgz_out: &Path,
    feed_out: &Path,
    srcs: &[std::path::PathBuf],
) -> io::Result<String> {
    if let Some(error) = npm_access_error(package, access)
        .or_else(|| npm_provenance_error(access, registry, provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
    }
    let mut members: std::collections::BTreeMap<String, (Vec<u8>, u32)> =
        std::collections::BTreeMap::new();
    for (src, rel) in npm_package_paths(srcs)? {
        let data = std::fs::read(&src)?;
        let member = format!("package/{rel}");
        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let bits = std::fs::metadata(&src)?.permissions().mode();
                if bits & 0o111 != 0 { 0o755 } else { 0o644 }
            }
            #[cfg(not(unix))]
            {
                0o644
            }
        };
        members.insert(member, (data, mode));
    }
    let tar = tar_archive(
        members
            .iter()
            .map(|(member, (data, mode))| (member.as_str(), data.as_slice(), *mode)),
    )?;
    let gz = gzip_compress(&tar)?;
    if let Some(parent) = tgz_out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(tgz_out, &gz)?;
    let digest = dx_digest::sha256_file_hex(tgz_out)?;
    let names: Vec<String> = members.keys().cloned().collect();
    let tgz_base = basename(tgz_out)?;
    let feed = render_json(&serde_json::json!({
        "access": access,
        "files": names,
        "name": package,
        "provenance": provenance,
        "registry": registry,
        "sha256": digest,
        "tag": tag,
        "tarball": tgz_base,
    }))?;
    if let Some(parent) = feed_out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(feed_out, feed.as_bytes())?;
    Ok(digest)
}

/// Rejects an npm access mode the package name cannot publish with.
pub fn npm_access_error(package: &str, access: &str) -> Option<String> {
    let scoped = package.starts_with('@');
    if scoped
        && !package[1..].split_once('/').is_some_and(|(scope, name)| {
            !scope.is_empty() && !name.is_empty() && !name.contains('/')
        })
    {
        return Some(format!(
            "npm_deploy: invalid package '{package}': a scoped name is @scope/name"
        ));
    }
    if !scoped && package.contains('/') {
        return Some(format!(
            "npm_deploy: invalid package '{package}': only scoped names contain '/'"
        ));
    }
    match access {
        "public" => None,
        "restricted" if scoped => None,
        "restricted" => Some(format!(
            "npm_deploy: access 'restricted' needs a scoped package, got unscoped '{package}'"
        )),
        _ => Some(format!(
            "npm_deploy: invalid access '{access}': want 'public' or 'restricted'"
        )),
    }
}

fn npm_registry_host(registry: &str) -> &str {
    registry
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
}

/// Rejects a provenance policy the access mode or registry cannot honor.
pub fn npm_provenance_error(access: &str, registry: &str, provenance: &str) -> Option<String> {
    match provenance {
        "none" => None,
        "required" if access != "public" => Some(format!(
            "npm_deploy: provenance 'required' is not supported with access '{access}'; \
             use access 'public' or provenance 'none'"
        )),
        "required" if npm_registry_host(registry) != "registry.npmjs.org" => Some(format!(
            "npm_deploy: provenance 'required' is not supported by registry '{registry}'; \
             use https://registry.npmjs.org or provenance 'none'"
        )),
        "required" => None,
        _ => Some(format!(
            "npm_deploy: invalid provenance '{provenance}': want 'required' or 'none'"
        )),
    }
}

/// Rejects a required provenance policy when no signing identity is available.
pub fn npm_provenance_unavailable(provenance: &str) -> Option<String> {
    if provenance != "required" {
        return None;
    }
    let present = |key: &str| std::env::var_os(key).is_some_and(|value| !value.is_empty());
    if present("ACTIONS_ID_TOKEN_REQUEST_URL") || present("SIGSTORE_ID_TOKEN") {
        return None;
    }
    Some(
        "npm_deploy: provenance 'required' but no CI identity token is available \
         (ACTIONS_ID_TOKEN_REQUEST_URL or SIGSTORE_ID_TOKEN); publishing nothing"
            .to_owned(),
    )
}

/// Builds the npm publish arguments for one staged tarball.
pub fn npm_publish_args(
    tarball: &Path,
    tag: &str,
    access: &str,
    registry: &str,
    provenance: &str,
) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "publish".into(),
        tarball.into(),
        "--tag".into(),
        tag.into(),
        "--access".into(),
        access.into(),
    ];
    args.push(if provenance == "required" {
        "--provenance".into()
    } else {
        "--provenance=false".into()
    });
    args.push("--registry".into());
    args.push(registry.into());
    args
}

/// Lists one gzip tar member table.
pub fn npm_tar_members(tgz_path: &Path) -> io::Result<Vec<String>> {
    let file = std::fs::File::open(tgz_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut members = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        members.push(String::from_utf8_lossy(&entry.path_bytes()).into_owned());
    }
    Ok(members)
}

/// Renders one npmrc auth line.
pub fn npm_npmrc_line(host: &str, token: &str) -> String {
    format!("//{host}/:_authToken={token}\n")
}

/// Writes one owner-only npmrc in a private staged dir.
pub fn npm_write_npmrc(host: &str, token: &str) -> io::Result<std::path::PathBuf> {
    let dir = tempfile_stage_dir("npm-publish-")?;
    match write_secure_file(&dir, ".npmrc", &npm_npmrc_line(host, token)) {
        Ok(path) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(error)
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct NpmFeed {
    name: String,
    tag: String,
    registry: String,
    access: String,
    provenance: String,
    #[allow(dead_code)]
    tarball: String,
    sha256: String,
    files: Vec<String>,
}

fn parse_npm_feed(body: &str) -> io::Result<NpmFeed> {
    serde_json::from_str(body).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("feed JSON is invalid: {error}"),
        )
    })
}

/// Verifies one npm pack pair.
pub fn npm_verify_pack(
    tgz_path: &Path,
    feed_path: &Path,
) -> io::Result<(String, String, String, String, String)> {
    let body = std::fs::read_to_string(feed_path)?;
    let NpmFeed {
        name,
        tag,
        registry,
        access,
        provenance,
        sha256: expected,
        files,
        ..
    } = parse_npm_feed(&body)?;
    let actual = dx_digest::sha256_file_hex(tgz_path)?;
    if expected != actual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "checksum mismatch for {} (expected {expected}, actual {actual})",
                tgz_path.display(),
            ),
        ));
    }
    let members = npm_tar_members(tgz_path)?;
    if members != files {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pack members mismatch for {}: {members:?}",
                tgz_path.display()
            ),
        ));
    }
    if let Some(error) = npm_access_error(&name, &access)
        .or_else(|| npm_provenance_error(&access, &registry, &provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    }
    Ok((name, tag, registry, access, provenance))
}

/// Refuses a staged tarball whose sha256 differs from the feed's.
pub fn npm_check_staged(staged_tgz: &Path, feed_path: &Path) -> io::Result<()> {
    let expected = parse_npm_feed(&std::fs::read_to_string(feed_path)?)?.sha256;
    let actual = dx_digest::sha256_file_hex(staged_tgz)?;
    if expected != actual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "checksum mismatch for {} (expected {expected}, actual {actual})",
                staged_tgz.display(),
            ),
        ));
    }
    Ok(())
}

/// Resolves the npm pack pair from explicit runfiles keys.
pub fn npm_find_pack_inputs(
    feed_rloc: &str,
    tarball_rloc: &str,
) -> io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let feed_path = resolve_runfile(feed_rloc)?;
    let tgz_path = resolve_runfile(tarball_rloc)?;
    Ok((feed_path, tgz_path))
}

/// Runs the npm pack tool.
pub fn npm_pack_main(argv: &[String]) -> i32 {
    if argv.len() < 9 {
        eprintln!(
            "usage: npm_pack <package> <tag> <registry> <access> <provenance> <tgz_out> <feed_out> <src...>"
        );
        return 1;
    }
    let package = argv[1].clone();
    let tag = argv[2].clone();
    let registry = argv[3].clone();
    let access = argv[4].clone();
    let provenance = argv[5].clone();
    let tgz_out = std::path::PathBuf::from(&argv[6]);
    let feed_out = std::path::PathBuf::from(&argv[7]);
    let srcs: Vec<std::path::PathBuf> = argv[8..].iter().map(std::path::PathBuf::from).collect();
    match npm_create_pack(
        &package,
        &tag,
        &registry,
        &access,
        &provenance,
        &tgz_out,
        &feed_out,
        &srcs,
    ) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("npm_pack: {error}");
            1
        }
    }
}

/// Extra env names `npm` keeps: CA and node settings plus the CI identity for provenance.
const NPM_CHILD_ENV_EXTRA: &[&str] = &[
    "CI",
    "NODE_EXTRA_CA_CERTS",
    "NODE_OPTIONS",
    "ComSpec",
    "GITHUB_ACTIONS",
    "GITHUB_WORKFLOW_REF",
    "GITHUB_REPOSITORY",
    "GITHUB_REPOSITORY_ID",
    "GITHUB_REPOSITORY_OWNER_ID",
    "GITHUB_SERVER_URL",
    "GITHUB_EVENT_NAME",
    "GITHUB_REF",
    "GITHUB_SHA",
    "GITHUB_RUN_ID",
    "GITHUB_RUN_ATTEMPT",
    "RUNNER_ENVIRONMENT",
    "ACTIONS_ID_TOKEN_REQUEST_URL",
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "GITLAB_CI",
    "SIGSTORE_ID_TOKEN",
    "CI_",
    "GITLAB_",
];

/// Runs npm publish with a temporary owner-only npmrc, in a cleared env.
pub fn npm_run_publish(npm: &Path, host: &str, token: &str, args: &[std::ffi::OsString]) -> i32 {
    let npmrc = match npm_write_npmrc(host, token) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let output = scrubbed_command(npm, &[PUBLISH_CHILD_ENV_KEEP, NPM_CHILD_ENV_EXTRA], &[])
        .args(args)
        .arg("--userconfig")
        .arg(&npmrc)
        .output();
    if let Some(dir) = npmrc.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    match output {
        Ok(out) => {
            print_redacted(&out, token);
            if out.status.success() {
                0
            } else {
                out.status.code().unwrap_or(1)
            }
        }
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            1
        }
    }
}

/// Owner approval gate for npm_deploy.
pub(crate) const NPM_GATE: OwnerGate = OwnerGate {
    tool: "npm_deploy",
    action: "publish",
    credentials: &["NPM_TOKEN"],
    approved: "NPM_PUBLISH_APPROVED",
};

/// Returns the declaration lines of an api-extractor report; comments, fences and headings are dropped.
fn api_report_declarations(report: &str) -> Vec<&str> {
    report
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !["//", "```", "#", "("]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
        })
        .collect()
}

/// Counts the declarations of the published report that the new report no longer has.
fn count_api_breaking(published: &str, staged: &str) -> usize {
    let staged = api_report_declarations(staged);
    api_report_declarations(published)
        .into_iter()
        .filter(|line| !staged.contains(line))
        .count()
}

/// Returns the version and the `types` entry of an extracted package.
fn package_version_and_types(package_dir: &Path) -> Result<(String, Option<String>), String> {
    let body = std::fs::read_to_string(package_dir.join("package.json"))
        .map_err(|error| format!("package.json: {error}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("package.json: {error}"))?;
    let version = json["version"].as_str().unwrap_or("").to_owned();
    let types = json["types"]
        .as_str()
        .or_else(|| json["typings"].as_str())
        .map(|types| types.trim_start_matches("./").to_owned());
    Ok((version, types))
}

/// Runs api-extractor on one extracted package and returns its report.
fn extract_api_report(
    api_extractor: &Path,
    work: &Path,
    package_dir: &Path,
    types: &str,
) -> Result<String, String> {
    let report_dir = work.join("report");
    let config = serde_json::json!({
        "projectFolder": package_dir,
        "mainEntryPointFilePath": package_dir.join(types),
        "compiler": {
            "overrideTsconfig": {
                "compilerOptions": {"skipLibCheck": true},
                "include": [],
            },
        },
        "apiReport": {
            "enabled": true,
            "reportFolder": report_dir,
            "reportTempFolder": work.join("temp"),
        },
        "docModel": {"enabled": false},
        "dtsRollup": {"enabled": false},
        "tsdocMetadata": {"enabled": false},
    });
    let config_path = work.join("api-extractor.json");
    std::fs::write(&config_path, config.to_string()).map_err(|error| error.to_string())?;
    let output = scrubbed_command(api_extractor, &[PUBLISH_CHILD_ENV_KEEP], &[])
        .args(["run", "--local", "--config"])
        .arg(&config_path)
        .current_dir(package_dir)
        .output()
        .map_err(|error| format!("api-extractor: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "api-extractor failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    let report = std::fs::read_dir(&report_dir)
        .map_err(|error| format!("api-extractor wrote no report: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(".api.md"))
        .ok_or("api-extractor wrote no report")?;
    std::fs::read_to_string(report).map_err(|error| error.to_string())
}

/// Downloads the latest published tarball of `name` into `dir`; None when the package is unpublished.
fn npm_pack_latest(npm: &Path, name: &str, dir: &Path) -> Option<std::path::PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let output = scrubbed_command(npm, &[PUBLISH_CHILD_ENV_KEEP, NPM_CHILD_ENV_EXTRA], &[])
        .args(["pack", &format!("{name}@latest"), "--pack-destination"])
        .arg(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "tgz"))
}

/// Tools and settings of the npm API compatibility check.
pub struct NpmApiTools<'a> {
    pub mode: &'a str,
    pub npm: Option<&'a Path>,
    pub api_extractor: Option<&'a Path>,
}

/// Diffs the api-extractor report of the staged tarball with the latest published one.
/// A missing tool, a missing `types` entry or an unpublished package skips with a note.
fn npm_api_check(staged_tgz: &Path, name: &str, tools: &NpmApiTools) -> Result<(), String> {
    let mode = ApiCompatMode::parse(if tools.mode.is_empty() {
        "off"
    } else {
        tools.mode
    })
    .ok_or_else(|| format!("npm_deploy: invalid api_compat '{}'", tools.mode))?;
    if mode == ApiCompatMode::Off {
        return Ok(());
    }
    let (Some(npm), Some(api_extractor)) = (tools.npm, tools.api_extractor) else {
        println!("api compat: npm or api-extractor not found on PATH; skipped");
        return Ok(());
    };
    let work = tempfile_stage_dir("npm-api-").map_err(|error| format!("npm_deploy: {error}"))?;
    let result = npm_api_diff(staged_tgz, name, npm, api_extractor, mode, &work);
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn npm_api_diff(
    staged_tgz: &Path,
    name: &str,
    npm: &Path,
    api_extractor: &Path,
    mode: ApiCompatMode,
    work: &Path,
) -> Result<(), String> {
    let staged_dir = work.join("staged");
    archive_extract(staged_tgz, &staged_dir).map_err(|error| format!("npm_deploy: {error}"))?;
    let staged_pkg = staged_dir.join("package");
    let (version, types) =
        package_version_and_types(&staged_pkg).map_err(|error| format!("npm_deploy: {error}"))?;
    let Some(types) = types else {
        println!("api compat: package has no types entry; skipped");
        return Ok(());
    };
    let Some(published_tgz) = npm_pack_latest(npm, name, &work.join("download")) else {
        println!("api compat: {name} is not published; skipped (first release)");
        return Ok(());
    };
    let published_dir = work.join("published");
    archive_extract(&published_tgz, &published_dir)
        .map_err(|error| format!("npm_deploy: {error}"))?;
    let published_pkg = published_dir.join("package");
    let (previous, published_types) = package_version_and_types(&published_pkg)
        .map_err(|error| format!("npm_deploy: {error}"))?;
    let Some(published_types) = published_types else {
        println!("api compat: published {previous} has no types entry; skipped");
        return Ok(());
    };
    let staged_report = extract_api_report(api_extractor, &staged_dir, &staged_pkg, &types)
        .map_err(|error| format!("npm_deploy: api compat: {error}"))?;
    let published_report = extract_api_report(
        api_extractor,
        &published_dir,
        &published_pkg,
        &published_types,
    )
    .map_err(|error| format!("npm_deploy: api compat: {error}"))?;
    let breaking = count_api_breaking(&published_report, &staged_report);
    println!("api compat: {breaking} removed or changed declarations against {previous}");
    match breaking_refusal(mode, required_bump(&previous, &version), breaking) {
        Some(reason) => Err(format!(
            "npm_deploy: api compat: {reason} ({previous} -> {version})"
        )),
        None => Ok(()),
    }
}

/// Runs the npm deploy tool.
pub fn npm_deploy_main(argv: &[String], feed_rloc: &str, tarball_rloc: &str) -> i32 {
    npm_deploy_main_checked(argv, feed_rloc, tarball_rloc, "off")
}

/// Runs the npm deploy tool with an API compatibility mode for the dry run.
pub fn npm_deploy_main_checked(
    argv: &[String],
    feed_rloc: &str,
    tarball_rloc: &str,
    api_compat: &str,
) -> i32 {
    if std::env::var_os("NPM_PUBLISH_DRY_RUN").is_some() {
        eprintln!(
            "npm_deploy: NPM_PUBLISH_DRY_RUN is not supported; publish with dx deploy --apply"
        );
        return 1;
    }
    let Some(run) = DeployRun::start(
        "npm_deploy",
        "the package is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let (feed_path, tgz_path) = match npm_find_pack_inputs(feed_rloc, tarball_rloc) {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let (name, tag, registry, access, provenance) = match npm_verify_pack(&tgz_path, &feed_path) {
        Ok(triple) => triple,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    if !NPM_GATE.requested(env_or_empty) {
        let (npm, api_extractor) = (which_on_path("npm"), which_on_path("api-extractor"));
        let tools = NpmApiTools {
            mode: api_compat,
            npm: npm.as_deref(),
            api_extractor: api_extractor.as_deref(),
        };
        if let Err(message) = npm_api_check(&tgz_path, &name, &tools) {
            eprintln!("{message}");
            return 1;
        }
    }
    let tgz_base = match basename(&tgz_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let feed_base = match basename(&feed_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let stem = tgz_base
        .strip_suffix(".tgz")
        .unwrap_or(&tgz_base)
        .to_owned();
    let feed_dir = outdir.join(stem.clone() + "-feed");
    if std::fs::create_dir_all(&feed_dir).is_err() {
        eprintln!("npm_deploy: cannot create {}", feed_dir.display());
        return 1;
    }
    for (src, base) in [(&tgz_path, &tgz_base), (&feed_path, &feed_base)] {
        if dx_atomic_fs::copy_atomic(src, &outdir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
        if dx_atomic_fs::copy_atomic(src, &feed_dir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
    }
    let profile = deploy_profile();
    println!("npm_deploy: released {name} (tag {tag}, access {access}, profile: {profile})");
    println!("  registry: {registry}");
    println!("  provenance: {provenance}");
    println!("  tarball: {}", outdir.join(&tgz_base).display());
    println!("  feed: {}", feed_dir.display());
    if NPM_GATE.requested(env_or_empty) {
        let secrets = match NPM_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        if let Some(error) = npm_provenance_unavailable(&provenance) {
            eprintln!("{error}");
            return 1;
        }
        let staged_tgz = outdir.join(&tgz_base);
        if let Err(error) = npm_check_staged(&staged_tgz, &feed_path) {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
        let Some(npm) = which_on_path("npm") else {
            eprintln!("npm_deploy: 'npm' CLI not found on PATH");
            return 1;
        };
        let host = npm_registry_host(&registry);
        if host.is_empty() {
            eprintln!("npm_deploy: invalid registry '{registry}'");
            return 1;
        }
        return npm_run_publish(
            &npm,
            host,
            secrets.get("NPM_TOKEN"),
            &npm_publish_args(&staged_tgz, &tag, &access, &registry, &provenance),
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn types_package(version: &str, index: &str, dst: &Path) {
        let manifest =
            format!("{{\"name\":\"pkg\",\"version\":\"{version}\",\"types\":\"./index.d.ts\"}}");
        archive_tree(
            &[
                ("package/package.json", manifest.as_bytes(), 0o644),
                ("package/index.d.ts", index.as_bytes(), 0o644),
            ],
            dst,
        )
        .expect("package tarball");
    }

    #[cfg(unix)]
    #[test]
    fn api_check_fails_on_removed_export_unless_major() {
        let scratch = scratch_dir();
        let dir = scratch.path();
        let published = dir.join("published.tgz");
        types_package(
            "1.2.0",
            "export declare function a(): void;\nexport declare function b(): void;\n",
            &published,
        );
        let fake_npm = dir.join("npm");
        std::fs::write(
            &fake_npm,
            format!(
                "#!/bin/sh\ncp {} \"$4/pkg-1.2.0.tgz\"\n",
                published.display()
            ),
        )
        .expect("fake npm");
        set_mode(&fake_npm, 0o755);
        let fake_extractor = dir.join("api-extractor");
        std::fs::write(
            &fake_extractor,
            "#!/bin/sh\nmkdir -p ../report\ncp index.d.ts ../report/pkg.api.md\n",
        )
        .expect("fake extractor");
        set_mode(&fake_extractor, 0o755);
        let run = |version: &str, mode: &str| {
            let staged = dir.join(format!("staged-{version}.tgz"));
            types_package(version, "export declare function a(): void;\n", &staged);
            npm_api_check(
                &staged,
                "pkg",
                &NpmApiTools {
                    mode,
                    npm: Some(&fake_npm),
                    api_extractor: Some(&fake_extractor),
                },
            )
        };
        let refused = run("1.3.0", "fail").expect_err("minor bump refused");
        assert!(refused.contains("1.2.0 -> 1.3.0"), "{refused}");
        assert!(run("2.0.0", "fail").is_ok());
        assert!(run("1.3.0", "warn").is_ok());
        assert!(run("1.3.0", "off").is_ok());
    }

    #[test]
    fn api_check_skips_without_tools() {
        let tools = NpmApiTools {
            mode: "fail",
            npm: None,
            api_extractor: None,
        };
        assert!(npm_api_check(Path::new("missing.tgz"), "pkg", &tools).is_ok());
    }

    #[test]
    fn api_report_diff_counts_removed_and_changed_declarations() {
        let old = "## API Report\n\n```ts\n\n// @public\nexport function a(x: string): void;\n\n// @public\nexport function b(): void;\n\n```\n";
        let new = "```ts\n// @public\nexport function a(x: number): void;\n\n// @public\nexport function b(): void;\nexport function c(): void;\n```\n";
        assert_eq!(count_api_breaking(old, new), 1);
        assert_eq!(count_api_breaking(new, old), 2);
    }

    #[test]
    fn npm_pack_and_verify_round_trip() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let digest = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(digest, dx_digest::sha256_file_hex(&tgz).expect("hex"));
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, vec!["package/package.json".to_owned()]);
        assert_eq!(
            npm_npmrc_line("registry.npmjs.org", "tok"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        let body = std::fs::read_to_string(&feed).expect("feed");
        assert!(body.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).expect("feed parses"),
            serde_json::json!({
                "access": "public",
                "files": ["package/package.json"],
                "name": "npm-demo",
                "provenance": "required",
                "registry": "https://registry.npmjs.org",
                "sha256": digest,
                "tag": "latest",
                "tarball": "npm_demo.tgz",
            })
        );
    }

    #[test]
    fn npm_tar_members_rejects_a_bad_header_checksum() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let good = scratch.path().join("good.tgz");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &good,
            &scratch.path().join("good.feed.json"),
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(npm_tar_members(&good).expect("members").len(), 1);
        let mut tar = Vec::new();
        use std::io::Read as _;
        flate2::read::GzDecoder::new(std::fs::File::open(&good).expect("open tgz"))
            .read_to_end(&mut tar)
            .expect("decode");
        tar[100] ^= 1;
        let bad = scratch.path().join("bad.tgz");
        std::fs::write(&bad, gzip_compress(&tar).expect("gz")).expect("write bad");
        assert!(npm_tar_members(&bad).is_err());
    }

    fn nested_npm_package(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                "package.json",
                "{\"name\": \"npm-nested\", \"version\": \"1.0.0\", \"main\": \"dist/index.js\", \"types\": \"dist/index.d.ts\", \"exports\": {\".\": \"./dist/index.js\", \"./util\": \"./dist/util/index.js\"}, \"bin\": {\"npm-nested\": \"bin/cli.js\"}}\n",
            ),
            write_crate_file(
                root,
                "dist/index.js",
                "const util = require('./util');\nmodule.exports = { value: 41 + util.one };\n",
            ),
            write_crate_file(root, "dist/index.d.ts", "export const value: number;\n"),
            write_crate_file(root, "dist/util/index.js", "module.exports = { one: 1 };\n"),
            write_crate_file(
                root,
                "bin/cli.js",
                "#!/usr/bin/env node\nconsole.log(require('../dist').value);\n",
            ),
        ]
    }

    fn pack_nested_npm(scratch: &Path) -> std::path::PathBuf {
        let srcs = nested_npm_package(&scratch.join("in/pkg"));
        let tgz = scratch.join("nested.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.join("nested.feed.json"),
            &srcs,
        )
        .expect("pack");
        tgz
    }

    #[test]
    fn npm_pack_preserves_package_paths() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        assert_eq!(
            npm_tar_members(&tgz).expect("members"),
            vec![
                "package/bin/cli.js".to_owned(),
                "package/dist/index.d.ts".to_owned(),
                "package/dist/index.js".to_owned(),
                "package/dist/util/index.js".to_owned(),
                "package/package.json".to_owned(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_pack_keeps_executable_modes() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = scratch_dir();
        let srcs = nested_npm_package(&scratch.path().join("in/pkg"));
        let cli = &srcs[4];
        std::fs::set_permissions(cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let tgz = scratch.path().join("modes.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.path().join("modes.feed.json"),
            &srcs,
        )
        .expect("pack");
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(&tgz).expect("open"),
        ));
        let mut modes = std::collections::BTreeMap::new();
        for entry in archive.entries().expect("entries") {
            let entry = entry.expect("entry");
            let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            modes.insert(name, entry.header().mode().expect("mode"));
        }
        assert_eq!(modes["package/bin/cli.js"], 0o755);
        assert_eq!(modes["package/dist/index.js"], 0o644);
    }

    #[test]
    fn npm_pack_allows_equal_basenames_in_different_directories() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let members = npm_tar_members(&tgz).expect("members");
        let indexes = members.iter().filter(|m| m.ends_with("/index.js")).count();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn npm_package_paths_reject_collisions_and_escapes() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let manifest = write_crate_file(&root, "package.json", "{}");
        let outside = write_crate_file(scratch.path(), "other/index.js", "");
        let error = npm_package_paths(&[manifest.clone(), outside]).expect_err("outside");
        assert!(
            error.to_string().contains("outside the package root"),
            "{error}"
        );
        write_crate_file(&root, "index.js", "");
        let error = npm_package_paths(&[manifest.clone(), root.join("src/../index.js")])
            .expect_err("dotdot");
        assert!(error.to_string().contains("unsafe package path"), "{error}");
        let error = npm_package_paths(&[
            manifest.clone(),
            root.join("index.js"),
            root.join("index.js"),
        ])
        .expect_err("dup");
        assert!(error.to_string().contains("path collision"), "{error}");
        let error = npm_package_paths(&[manifest.clone(), root.join("src"), root.join("src/a.js")])
            .expect_err("file+dir");
        assert!(
            error.to_string().contains("both a file and a directory"),
            "{error}"
        );
        let error = npm_package_paths(&[root.join("index.js")]).expect_err("no manifest");
        assert!(error.to_string().contains("no package.json"), "{error}");
        let error = npm_package_paths(&[
            write_crate_file(&root, "a/package.json", "{}"),
            write_crate_file(&root, "b/package.json", "{}"),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
        assert!(npm_package_paths(&[]).is_err());
    }

    #[test]
    fn npm_pack_installs_into_a_local_consumer() {
        let ok = |program: &str| {
            std::process::Command::new(program)
                .arg("--version")
                .output()
                .map(|out| out.status.success())
                .unwrap_or(false)
        };
        if !ok("node") || !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let consumer = scratch.path().join("consumer");
        write_crate_file(
            &consumer,
            "package.json",
            "{\"name\": \"consumer\", \"version\": \"0.0.0\", \"private\": true}\n",
        );
        let install = std::process::Command::new("npm")
            .args(["install", "--offline", "--no-audit", "--no-fund"])
            .arg(&tgz)
            .env("npm_config_cache", scratch.path().join("npm-cache"))
            .current_dir(&consumer)
            .output()
            .expect("npm install");
        assert!(
            install.status.success(),
            "{}",
            String::from_utf8_lossy(&install.stderr)
        );
        let run = std::process::Command::new("node")
            .args([
                "-e",
                "console.log(require('npm-nested').value + require('npm-nested/util').one)",
            ])
            .current_dir(&consumer)
            .output()
            .expect("node");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "43");
        assert!(
            consumer
                .join("node_modules/npm-nested/dist/index.d.ts")
                .is_file()
        );
        assert!(consumer.join("node_modules/.bin/npm-nested").exists());
    }

    #[test]
    fn npm_feed_parser_reads_escapes_brackets_and_unicode() {
        let body = r#"{ "tarball":"a.tgz", "tag":"t", "sha256":"d", "registry":"r", "access":"public", "provenance":"none",
            "name": "a\"b\\c\nd", "extra": [1, {"x": "]"}],
            "files": ["package/[id].js", "package/]x[.js", "y\\z", "w\"q", "\u00e6\u0041", "æ"] }"#;
        let feed = parse_npm_feed(body).expect("feed");
        assert_eq!(feed.name, "a\"b\\c\nd");
        assert_eq!(
            feed.files,
            [
                "package/[id].js",
                "package/]x[.js",
                "y\\z",
                "w\"q",
                "\u{e6}A",
                "\u{e6}"
            ]
        );
    }

    #[test]
    fn npm_feed_parser_rejects_bad_documents() {
        let good = r#"{"access": "public", "files": [], "name": "n", "provenance": "none", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a.tgz"}"#;
        parse_npm_feed(good).expect("good");
        let bad = [
            "",
            "[]",
            r#"{"name": "#,
            r#"{"files": ["a"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t"}"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": 7, "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [1], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": 7, "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": "n", "name": "m", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "files": ["x"], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
        ];
        for body in bad {
            let error = parse_npm_feed(body).expect_err(body);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{body}");
        }
    }

    #[test]
    fn npm_find_pack_inputs_reads_manifest_entries() {
        if env_test_in_child("npm::tests::npm_find_pack_inputs_reads_manifest_entries") {
            return;
        }
        let scratch = scratch_dir();
        let tgz = scratch.path().join("npm_demo.tgz");
        std::fs::write(&tgz, b"tarball-bytes").expect("write tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        std::fs::write(
            &feed,
            b"{\"name\": \"npm-demo\", \"tarball\": \"npm_demo.tgz\"}\n",
        )
        .expect("write feed");
        let manifest = scratch.path().join("manifest.txt");
        std::fs::write(
            &manifest,
            format!(
                "_main/npm/nested/npm_demo.feed.json {}\n_main/npm/nested/npm_demo.tgz {}\n",
                feed.display(),
                tgz.display()
            ),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let (found_feed, found_tgz) = npm_find_pack_inputs(
                    "_main/npm/nested/npm_demo.feed.json",
                    "_main/npm/nested/npm_demo.tgz",
                )
                .expect("pack pair");
                assert_eq!(found_feed, feed);
                assert_eq!(found_tgz, tgz);
                let error = npm_find_pack_inputs("npm_demo.feed.json", "npm_demo.tgz")
                    .expect_err("basename is not a key");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert!(error.to_string().contains("npm_demo.feed.json"));
            },
        );
        let empty_dir = scratch.path().join("empty-runfiles");
        std::fs::create_dir_all(&empty_dir).expect("empty runfiles");
        with_env(
            &[
                ("RUNFILES_DIR", Some(empty_dir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let error = npm_find_pack_inputs("_main/a.feed.json", "_main/a.tgz")
                    .expect_err("no feed under runfiles");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn npmrc_written_with_owner_only_permissions() {
        let path = npm_write_npmrc("registry.npmjs.org", "tok").expect("npmrc");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read npmrc"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "npmrc must stay owner-only");
        }
        let parent = path.parent().expect("npmrc parent");
        assert_ne!(parent, std::env::temp_dir().as_path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(parent)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "npmrc dir must stay owner-only");
        }
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn npm_access_error_covers_scope_and_mode() {
        assert!(npm_access_error("npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "restricted").is_none());
        let unscoped = npm_access_error("npm-demo", "restricted").expect("unscoped");
        assert!(unscoped.contains("needs a scoped package"), "{unscoped}");
        let unknown = npm_access_error("npm-demo", "private").expect("unknown");
        assert!(unknown.contains("invalid access"), "{unknown}");
        for bad in ["@scope", "@/name", "@a/b/c", "a/b"] {
            assert!(npm_access_error(bad, "public").is_some(), "{bad}");
        }
    }

    #[test]
    fn npm_publish_args_carry_access_and_no_secret() {
        let tgz = Path::new("/stage/p.tgz");
        let public = npm_publish_args(tgz, "latest", "public", "https://r.example", "required");
        let public: Vec<String> = public
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            public,
            [
                "publish",
                "/stage/p.tgz",
                "--tag",
                "latest",
                "--access",
                "public",
                "--provenance",
                "--registry",
                "https://r.example"
            ]
        );
        let restricted = npm_publish_args(tgz, "next", "restricted", "https://r.example", "none");
        let restricted: Vec<String> = restricted
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(restricted.contains(&"restricted".to_owned()));
        assert!(!restricted.contains(&"--provenance".to_owned()));
        assert!(restricted.contains(&"--provenance=false".to_owned()));
    }

    #[test]
    fn npm_pack_rejects_restricted_unscoped_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        let error = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("mismatch");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_verify_pack_reports_restricted_access() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(
            &src,
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect("pack");
        let (name, _, registry, access, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "@s/npm-demo");
        assert_eq!(registry, "https://registry.npmjs.org");
        assert_eq!(access, "restricted");
        let body = std::fs::read_to_string(&feed).expect("feed");
        std::fs::write(&feed, body.replace("\"@s/npm-demo\"", "\"npm-demo\"")).expect("tamper");
        let error = npm_verify_pack(&tgz, &feed).expect_err("tampered");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
    }

    #[test]
    fn npm_deploy_rejects_former_dry_run_request() {
        with_env(&[("NPM_PUBLISH_DRY_RUN", Some("1"))], || {
            assert_eq!(
                npm_deploy_main(&["prog".to_owned()], "a.feed.json", "a.tgz"),
                1
            );
        });
    }

    #[test]
    fn npm_cli_pack_and_publish_dry_run_accept_the_staged_package() {
        let ok = |tool: &str| {
            std::process::Command::new(tool)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let pkg = scratch.path().join("pkg");
        std::fs::create_dir_all(&pkg).expect("pkg");
        std::fs::write(
            pkg.join("package.json"),
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[pkg.join("package.json")],
        )
        .expect("pack");
        let run = |args: Vec<std::ffi::OsString>| {
            std::process::Command::new("npm")
                .args(args)
                .env("npm_config_cache", scratch.path().join("npm-cache"))
                .env("npm_config_userconfig", scratch.path().join("npmrc"))
                .env("npm_config_update_notifier", "false")
                .current_dir(scratch.path())
                .output()
                .expect("npm")
        };
        let packed = run(vec![
            "pack".into(),
            "--dry-run".into(),
            "--json".into(),
            pkg.clone().into(),
        ]);
        assert!(packed.status.success(), "{packed:?}");
        let mut args = npm_publish_args(
            &tgz,
            "latest",
            "restricted",
            "https://registry.npmjs.org",
            "none",
        );
        args.push("--dry-run".into());
        let published = run(args);
        let text = String::from_utf8_lossy(&published.stderr).into_owned()
            + &String::from_utf8_lossy(&published.stdout);
        assert!(published.status.success(), "{text}");
        assert!(text.contains("@s/npm-demo"), "{text}");
    }

    #[test]
    fn npm_pack_main_requires_full_argv() {
        let short: Vec<String> = ["npm_pack", "a", "b", "c", "d", "e"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(npm_pack_main(&short), 1);
    }

    #[test]
    fn npm_bracketed_filename_round_trips_through_verify() {
        let scratch = scratch_dir();
        let src = scratch.path().join("[id].js");
        std::fs::write(&src, b"module.exports = 1;\n").expect("write src");
        let manifest = scratch.path().join("package.json");
        std::fs::write(
            &manifest,
            b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n",
        )
        .expect("write manifest");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            &[manifest, src],
        )
        .expect("pack");
        let (name, _, _, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, ["package/[id].js", "package/package.json"]);

        let body = std::fs::read_to_string(&feed).expect("read feed");
        let reordered = scratch.path().join("reordered.feed.json");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
        std::fs::write(&reordered, serde_json::to_string(&parsed).expect("compact"))
            .expect("write");
        npm_verify_pack(&tgz, &reordered).expect("whitespace does not matter");

        let tampered = scratch.path().join("tampered.feed.json");
        std::fs::write(&tampered, body.replace("package/[id].js", "package/[x].js"))
            .expect("write");
        let error = npm_verify_pack(&tgz, &tampered).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn npm_verify_and_pack_report_feed_problems() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");

        let body = std::fs::read_to_string(&feed).expect("read feed");
        for field in ["name", "tag", "registry", "tarball", "sha256", "files"] {
            let stripped = body
                .lines()
                .filter(|line| !line.contains(&format!("\"{field}\"")))
                .collect::<Vec<_>>()
                .join("\n");
            let path = scratch.path().join(format!("missing_{field}.feed.json"));
            std::fs::write(&path, stripped).expect("write feed");
            let error = npm_verify_pack(&tgz, &path).expect_err(field);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{field}");
        }

        let wrong_members = scratch.path().join("wrong_members.feed.json");
        std::fs::write(
            &wrong_members,
            body.replace("package/package.json", "package/absent.json"),
        )
        .expect("write feed");
        let error = npm_verify_pack(&tgz, &wrong_members).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn npm_pack_main_packs_and_reports_failures() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let argv = vec![
            "npm_pack".to_owned(),
            "npm-demo".to_owned(),
            "latest".to_owned(),
            "https://registry.npmjs.org".to_owned(),
            "public".to_owned(),
            "required".to_owned(),
            tgz.to_string_lossy().into_owned(),
            feed.to_string_lossy().into_owned(),
            src.to_string_lossy().into_owned(),
        ];
        assert_eq!(npm_pack_main(&argv), 0);
        assert!(tgz.is_file());
        assert!(feed.is_file());
        let mut missing_src = argv.clone();
        missing_src.pop();
        missing_src.push(
            scratch
                .path()
                .join("absent.json")
                .to_string_lossy()
                .into_owned(),
        );
        assert_eq!(npm_pack_main(&missing_src), 1);
    }

    fn stage_npm_demo(root: &Path, provenance: &str) -> std::path::PathBuf {
        let runfiles = root.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let src = runfiles.join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let bracketed = runfiles.join("[id].js");
        std::fs::write(&bracketed, b"module.exports = 1;\n").expect("write bracketed");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            provenance,
            &runfiles.join("npm_demo.tgz"),
            &runfiles.join("npm_demo.feed.json"),
            &[src, bracketed],
        )
        .expect("pack");
        runfiles
    }

    #[test]
    fn npm_deploy_main_releases_a_staged_pack() {
        if env_test_in_child("npm::tests::npm_deploy_main_releases_a_staged_pack") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_BUILD_MODE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("DX_DEPLOY_LIVE", None),
                ("NPM_PUBLISH_APPROVED", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    0
                );
            },
        );
        assert!(outdir.join("npm_demo.tgz").is_file());
        assert!(
            outdir
                .join("npm_demo-feed")
                .join("npm_demo.feed.json")
                .is_file()
        );
        assert!(
            npm_tar_members(&outdir.join("npm_demo.tgz"))
                .expect("members")
                .contains(&"package/[id].js".to_owned())
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn npm_provenance_policy_rejects_unsupported_combinations() {
        let npmjs = "https://registry.npmjs.org";
        assert!(npm_provenance_error("public", npmjs, "required").is_none());
        assert!(npm_provenance_error("public", npmjs, "none").is_none());
        assert!(npm_provenance_error("restricted", "https://npm.example.com", "none").is_none());
        let restricted = npm_provenance_error("restricted", npmjs, "required").expect("restricted");
        assert!(restricted.contains("not supported with access 'restricted'"));
        let private = npm_provenance_error("public", "https://npm.example.com/repo", "required")
            .expect("registry");
        assert!(private.contains("not supported by registry"));
        let unknown = npm_provenance_error("public", npmjs, "auto").expect("unknown");
        assert!(unknown.contains("want 'required' or 'none'"));
    }

    #[test]
    fn npm_create_pack_refuses_unsupported_provenance_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"x\", \"version\": \"1.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("x.tgz");
        let feed = scratch.path().join("x.feed.json");
        let error = npm_create_pack(
            "@s/x",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "required",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("restricted provenance");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_publish_args_state_the_provenance_policy() {
        let tgz = Path::new("x.tgz");
        let required = npm_publish_args(tgz, "latest", "public", "https://r", "required");
        assert!(required.iter().any(|arg| arg == "--provenance"));
        let none = npm_publish_args(tgz, "latest", "restricted", "https://r", "none");
        assert!(none.iter().any(|arg| arg == "--provenance=false"));
        assert!(!none.iter().any(|arg| arg == "--provenance"));
    }

    #[test]
    fn npm_provenance_unavailable_without_identity() {
        let none = [
            ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
            ("SIGSTORE_ID_TOKEN", None),
        ];
        with_env(&none, || {
            let error = npm_provenance_unavailable("required").expect("unavailable");
            assert!(error.contains("publishing nothing"));
            assert!(npm_provenance_unavailable("none").is_none());
        });
        with_env(
            &[
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || assert!(npm_provenance_unavailable("required").is_none()),
        );
    }

    #[cfg(unix)]
    fn fake_npm(dir: &Path, exit: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("npm");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\nexit {exit}\n"
        );
        std::fs::write(&path, script).expect("write npm");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[test]
    fn npm_check_staged_refuses_a_changed_tarball() {
        let scratch = scratch_dir();
        let tgz = scratch.path().join("a.tgz");
        std::fs::write(&tgz, b"good").expect("write");
        let sha = dx_digest::sha256_file_hex(&tgz).expect("sha");
        let feed = scratch.path().join("feed.json");
        std::fs::write(&feed, format!(
                r#"{{"access": "public", "files": [], "name": "n", "provenance": "none", "registry": "r", "sha256": "{sha}", "tag": "t", "tarball": "a.tgz"}}"#
            )).expect("feed");
        assert!(npm_check_staged(&tgz, &feed).is_ok());
        std::fs::write(&tgz, b"bad").expect("write");
        let error = npm_check_staged(&tgz, &feed).expect_err("mismatch");
        assert!(error.to_string().contains("checksum mismatch"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_passes_policy_and_keeps_the_token_out_of_args() {
        let scratch = scratch_dir();
        let npm = fake_npm(scratch.path(), 0);
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://registry.npmjs.org",
            "required",
        );
        let code = with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("NPM_TOKEN", Some("sekret")),
                ("GITHUB_TOKEN", Some("gh-sekret")),
                ("CI_JOB_TOKEN", Some("job-sekret")),
                ("CI_PROJECT_URL", Some("https://gitlab.example/p")),
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
            ],
            || npm_run_publish(&npm, "registry.npmjs.org", "sekret", &args),
        );
        assert_eq!(code, 0);
        let seen = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(seen.contains("--provenance\n"));
        assert!(seen.contains("--userconfig"));
        assert!(!seen.contains("sekret"));
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
        assert!(!env.contains("sekret"), "{env}");
        assert!(
            env.lines()
                .any(|line| line == "ACTIONS_ID_TOKEN_REQUEST_URL=https://ci.example/token")
        );
        assert!(
            env.lines()
                .any(|line| line == "CI_PROJECT_URL=https://gitlab.example/p")
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_surfaces_tool_failure_and_refusal() {
        let scratch = scratch_dir();
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://r",
            "required",
        );
        let refused = fake_npm(scratch.path(), 1);
        assert_eq!(npm_run_publish(&refused, "r", "tok", &args), 1);
        let failed = fake_npm(scratch.path(), 7);
        assert_eq!(npm_run_publish(&failed, "r", "tok", &args), 7);
        assert_eq!(
            npm_run_publish(&scratch.path().join("absent"), "r", "tok", &args),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_deploy_main_requires_owner_approval_for_live_publish() {
        if env_test_in_child("npm::tests::npm_deploy_main_requires_owner_approval_for_live_publish")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "none");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        fake_npm(&bin, 0);
        let args = bin.join("args.txt");
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        for (approved, want) in [(None, 1), (Some("0"), 1), (Some("true"), 1), (Some("1"), 0)] {
            let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
            let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("BUILD_WORKSPACE_DIRECTORY", None),
                    ("PATH", Some(path.as_str())),
                    ("DX_DEPLOY_LIVE", Some("1")),
                    ("NPM_PUBLISH_DRY_RUN", None),
                    ("NPM_TOKEN", Some("sekret")),
                    ("NPM_PUBLISH_APPROVED", approved),
                ],
                || {
                    assert_eq!(
                        npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                        want,
                        "approval {approved:?}"
                    );
                },
            );
            std::fs::remove_dir_all(&outdir).expect("cleanup");
            if want == 1 {
                assert!(!args.exists(), "npm ran with approval {approved:?}");
            }
        }
        let seen = std::fs::read_to_string(&args).expect("npm ran once approved");
        assert!(seen.contains("publish"));
        assert!(!seen.contains("sekret"));
    }

    #[test]
    fn npm_deploy_main_refuses_required_provenance_without_identity() {
        if env_test_in_child(
            "npm::tests::npm_deploy_main_refuses_required_provenance_without_identity",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("DX_DEPLOY_LIVE", Some("1")),
                ("NPM_TOKEN", Some("tok")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }
}
