//! crates.io publisher: vendor staging and the cargo publish.

use crate::api_compat::{
    ApiCompatMode, Baseline, Outcome, changes_or_failure, conclude, export_git_dir, fetch,
    latest_published, previous_git_tag, workspace_dir,
};
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty,
    exit_text, placeholder_refusal, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Maps crate input files to package-relative paths under the Cargo.toml directory.
pub fn crates_package_paths(
    crate_files: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    if crate_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crates vendor: need at least one crate source file",
        ));
    }
    package_relative_paths(crate_files, "Cargo.toml", "crates vendor")
}

/// Builds one crates vendor tree.
pub fn crates_build_vendor(
    crate_files: &[std::path::PathBuf],
    outdir: &Path,
    crate_name: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    let mapped = crates_package_paths(crate_files)?;
    let house = outdir.join(crate_name.to_owned() + "-vendor");
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

/// Owner approval gate for crates_deploy.
pub(crate) const CRATES_GATE: OwnerGate = OwnerGate {
    tool: "crates_deploy",
    action: "publish",
    credentials: &["CRATES_API_TOKEN"],
    approved: "CRATES_PUBLISH_APPROVED",
};

/// Extra env names cargo keeps beyond the publish base list.
const CARGO_CHILD_ENV_EXTRA: &[&str] = &["CARGO_HOME", "RUSTUP_HOME"];

/// Most cargo stderr lines shown when a publish fails.
const CARGO_FAILURE_LINES: usize = 40;

fn crates_publish_staged(
    program: &Path,
    files: &[std::path::PathBuf],
    staging: &Path,
    crate_name: &str,
    version: &str,
    token: &str,
) -> Result<(), String> {
    let mapped = crates_package_paths(files).map_err(|error| format!("crates_deploy: {error}"))?;
    crates_stage(&mapped, staging).map_err(|error| format!("crates_deploy: {error}"))?;
    let manifest = staging.join("Cargo.toml");
    if !manifest.is_file() {
        return Err("crates_deploy: staged crate is missing Cargo.toml; refusing".to_owned());
    }
    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    manifest_version(&text, crate_name, version)?;
    cargo_publish(program, &manifest, token)
}

/// Runs `cargo publish`; a failure carries cargo's last stderr lines.
fn cargo_publish(program: &Path, manifest: &Path, token: &str) -> Result<(), String> {
    let mut cmd = scrubbed_command(
        program,
        &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
        &[("CARGO_REGISTRY_TOKEN", token)],
    );
    cmd.arg("publish").arg("--manifest-path").arg(manifest);
    let output = run_publish(&mut cmd).map_err(|error| format!("crates_deploy: {error}"))?;
    if output.success() {
        return Ok(());
    }
    let mut message = format!("crates_deploy: cargo publish failed with {}", exit_text(&output));
    for line in cargo_failure_lines(&output.stderr, token) {
        message.push_str("\ncargo: ");
        message.push_str(&line);
    }
    Err(message)
}

/// Returns the last stderr lines with the token value removed.
fn cargo_failure_lines(stderr: &[u8], token: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(stderr);
    let text = if token.is_empty() {
        text.into_owned()
    } else {
        text.replace(token, "[redacted]")
    };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(CARGO_FAILURE_LINES)..]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

/// Returns the `[package]` name and version of a manifest; None when a field is not a plain string.
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

/// Returns the version to publish, refusing a manifest that is not the target crate or declares another version.
fn manifest_version(text: &str, crate_name: &str, version: &str) -> Result<String, String> {
    let (name, found) = manifest_identity(text);
    if name.as_deref() != Some(crate_name) {
        return Err(format!(
            "crates_deploy: Cargo.toml is {}, target is {crate_name}; refusing",
            name.unwrap_or_else(|| "(not a plain value)".to_owned())
        ));
    }
    resolve_version("crates_deploy", "Cargo.toml", found.as_deref(), version)
}

/// Reads the version to publish from the crate's Cargo.toml.
fn crates_version(
    files: &[std::path::PathBuf],
    crate_name: &str,
    version: &str,
) -> Result<String, String> {
    let mapped = crates_package_paths(files).map_err(|error| format!("crates_deploy: {error}"))?;
    let Some((manifest, _)) = mapped.iter().find(|(_, rel)| rel == "Cargo.toml") else {
        return Err("crates_deploy: crate has no Cargo.toml; refusing".to_owned());
    };
    let text = std::fs::read_to_string(manifest)
        .map_err(|error| format!("crates_deploy: cannot read Cargo.toml: {error}"))?;
    manifest_version(&text, crate_name, version)
}

fn crates_stage(mapped: &[(std::path::PathBuf, String)], staging: &Path) -> Result<(), String> {
    for (src, rel) in mapped {
        let dest = staging.join(rel);
        let staged = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| copy_verified(src, &dest).map(|_| ()));
        if staged.is_err() {
            return Err(format!("cannot stage {}", src.display()));
        }
    }
    Ok(())
}

/// Runs `cargo package --list` on a staged copy; a missing cargo skips the check.
fn crates_package_check(
    cargo: Option<&Path>,
    mapped: &[(std::path::PathBuf, String)],
) -> Result<(), String> {
    let Some(program) = cargo else {
        println!("crates_deploy: cargo not found on PATH; skipped cargo package --list check");
        return Ok(());
    };
    let staging =
        tempfile_stage_dir("crates-check-").map_err(|error| format!("crates_deploy: {error}"))?;
    let checked = crates_stage(mapped, &staging)
        .map_err(|error| format!("crates_deploy: {error}"))
        .and_then(|()| cargo_package_list(program, &staging.join("Cargo.toml")));
    let _ = std::fs::remove_dir_all(&staging);
    checked
}

/// Runs `cargo package --list` offline; a failure carries cargo's last stderr lines.
fn cargo_package_list(program: &Path, manifest: &Path) -> Result<(), String> {
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
    let output = run_publish(&mut cmd).map_err(|error| format!("crates_deploy: {error}"))?;
    if output.success() {
        return Ok(());
    }
    let mut message = format!(
        "crates_deploy: cargo package --list failed with {}",
        exit_text(&output)
    );
    for line in cargo_failure_lines(&output.stderr, "") {
        message.push_str("\ncargo: ");
        message.push_str(&line);
    }
    Err(message)
}

/// Where the sparse index entry of a crate lives.
fn index_path(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// Returns the highest published version below `next`; None when there is none.
fn published_below(crate_name: &str, next: &str) -> Result<Option<String>, String> {
    let url = format!("https://index.crates.io/{}", index_path(crate_name));
    Ok(fetch(&url, 64 << 20)?
        .and_then(|body| latest_published(&String::from_utf8_lossy(&body), next)))
}

/// Returns the `--- failure` lines of cargo-semver-checks output.
fn semver_changes(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("--- failure "))
        .map(|line| line.trim_end_matches(" ---").to_owned())
        .collect()
}

/// Compares the public API with the release `baseline` picks below `version`.
fn crates_api_check(
    program: &Path,
    mapped: &[(std::path::PathBuf, String)],
    version: &str,
    crate_name: &str,
    baseline: Baseline,
    crate_dir: &str,
) -> Result<Outcome, String> {
    let mut export_dir = None;
    let (previous, baseline_args) = if baseline == Baseline::Git {
        let tag = match previous_git_tag(&workspace_dir(), version) {
            Ok(Some(tag)) => tag,
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error)),
        };
        let dest = tempfile_stage_dir("crates-api-base-")
            .map_err(|error| format!("crates_deploy: {error}"))?;
        match export_git_dir(&workspace_dir(), crate_dir, &tag, &dest, "Cargo.toml") {
            Ok(true) => {}
            Ok(false) => {
                let _ = std::fs::remove_dir_all(&dest);
                return Ok(Outcome::Skipped(format!(
                    "no Cargo.toml at {crate_dir} in {tag}"
                )));
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dest);
                return Err(format!("crates_deploy: api compat: {error}"));
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
            Err(error) => return Ok(Outcome::Skipped(error)),
        }
    };
    let staging = match tempfile_stage_dir("crates-api-") {
        Ok(dir) => dir,
        Err(error) => {
            if let Some(dir) = &export_dir {
                let _ = std::fs::remove_dir_all(dir);
            }
            return Err(format!("crates_deploy: {error}"));
        }
    };
    let ran = crates_stage(mapped, &staging)
        .map_err(|error| format!("crates_deploy: {error}"))
        .and_then(|()| {
            let mut cmd = scrubbed_command(
                program,
                &[PUBLISH_CHILD_ENV_KEEP, CARGO_CHILD_ENV_EXTRA],
                &[],
            );
            cmd.args(["semver-checks", "check-release", "--manifest-path"])
                .arg(staging.join("Cargo.toml"))
                .args(&baseline_args);
            run_publish(&mut cmd).map_err(|error| format!("crates_deploy: {error}"))
        });
    let _ = std::fs::remove_dir_all(&staging);
    if let Some(dir) = &export_dir {
        let _ = std::fs::remove_dir_all(dir);
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

/// Runs the API check for `version` and prints its report.
fn crates_api_report(
    semver_checks: Option<&Path>,
    mapped: &[(std::path::PathBuf, String)],
    (crate_name, version): (&str, &str),
    (mode, baseline): (ApiCompatMode, Baseline),
    crate_dir: &str,
) -> Result<(), String> {
    if mode == ApiCompatMode::Off {
        return Ok(());
    }
    let outcome = match semver_checks {
        Some(program) => {
            crates_api_check(program, mapped, version, crate_name, baseline, crate_dir)?
        }
        None => Outcome::Skipped("cargo-semver-checks is not available".to_owned()),
    };
    conclude("crates_deploy", mode, version, outcome)
}

/// Runs the crates deploy tool without an API compatibility check.
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

/// API compatibility settings for `crates_main_checked`.
pub struct CratesApiCheck<'a> {
    pub api_compat: &'a str,
    pub api_baseline: &'a str,
    pub semver_checks_rloc: &'a str,
    pub crate_dir: &'a str,
}

/// Runs the crates deploy tool.
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
    if CRATES_GATE.requested(env_or_empty) {
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
        let _ = std::fs::remove_dir_all(&staging);
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
    let (Some(mode), Some(baseline)) = (
        ApiCompatMode::parse(api_compat),
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
    let checked = crates_package_paths(&files)
        .map_err(|error| format!("crates_deploy: {error}"))
        .and_then(|mapped| {
            crates_package_check(which_on_path("cargo").as_deref(), &mapped)?;
            crates_api_report(
                semver_checks.as_deref(),
                &mapped,
                (crate_name, version),
                (mode, baseline),
                crate_dir,
            )
        });
    if let Err(message) = checked {
        eprintln!("{message}");
        return 1;
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
            manifest_version(ok, "demo", "1.2.3"),
            Ok("1.2.3".to_owned())
        );
        assert_eq!(manifest_version(ok, "demo", ""), Ok("1.2.3".to_owned()));
        let wrong_name = manifest_version(ok, "other", "").expect_err("name refused");
        assert!(wrong_name.contains("Cargo.toml is demo, target is other"));
        let mismatch = manifest_version(ok, "demo", "2.0.0").expect_err("version refused");
        assert!(mismatch.contains("declares version 1.2.3, BUILD declares 2.0.0"));
        let workspace = "[package]\nname = \"demo\"\nversion.workspace = true\n";
        assert!(manifest_version(workspace, "demo", "").is_err());
        assert!(manifest_version("", "demo", "").is_err());
    }

    #[test]
    fn crates_vendor_builds_checksums() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join("Cargo.toml");
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
            "Cargo.toml": pairs["Cargo.toml"],
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
        assert_eq!(index["files"][0]["name"], "Cargo.toml");
        assert_eq!(index["files"][0]["sha256"], pairs["Cargo.toml"].as_str());
        assert!(crates_build_vendor(&[], &outdir, "crates_demo", "0.0.0").is_err());
    }

    #[test]
    fn crates_stage_copies_and_verifies_each_file() {
        let scratch = scratch_dir();
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mapped = crates_package_paths(&files).expect("paths");
        let staging = scratch.path().join("staging");
        crates_stage(&mapped, &staging).expect("stage");
        for (src, rel) in &mapped {
            assert_eq!(
                std::fs::read(staging.join(rel)).expect("staged"),
                std::fs::read(src).expect("source"),
            );
        }
        let missing = vec![(scratch.path().join("missing.rs"), "missing.rs".to_owned())];
        assert!(crates_stage(&missing, &staging).is_err());
    }

    #[test]
    fn crates_publish_staged_failure_is_reported_for_caller_cleanup() {
        let scratch = scratch_dir();
        let staging = scratch.path().join("crates-publish-x");
        let missing = vec![scratch.path().join("missing/Cargo.toml")];
        let error = crates_publish_staged(
            Path::new("cargo"),
            &missing,
            &staging,
            "crates_demo",
            "0.0.0",
            "t",
        )
        .expect_err("stage fails");
        assert!(error.starts_with("crates_deploy:"));
    }

    fn nested_crate(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                "Cargo.toml",
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
                "Cargo.toml",
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
                "Cargo.toml",
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
        let manifest = write_crate_file(&root, "Cargo.toml", "[package]\n");
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
        assert!(error.to_string().contains("no Cargo.toml"), "{error}");
        let error = crates_package_paths(&[
            write_crate_file(&root, "a/Cargo.toml", ""),
            write_crate_file(&root, "b/Cargo.toml", ""),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
    }

    #[test]
    fn cargo_failure_lines_keep_last_lines_and_hide_token() {
        let stderr: String = (1..=50).map(|n| format!("line {n} s3cret\n")).collect();
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
        let message = cargo_publish(&fake, &scratch.path().join("Cargo.toml"), "s3cret")
            .expect_err("publish fails");
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
        let message = crates_package_check(Some(&fake), &mapped).expect_err("rejected");
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
            "Cargo.toml",
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
        write_crate_file(&repo, "pkg/Cargo.toml", "[package]\nname = \"pkg\"\n");
        write_crate_file(&repo, "pkg/src/lib.rs", "pub fn hello() {}\n");
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "-m",
                "x",
            ],
            vec!["tag", "v1.2.0"],
        ] {
            dx_process::capture::run_checked(
                dx_process::capture::git(&repo).args(&args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }
        let files = nested_crate(&scratch.path().join("in/pkg"));
        let mapped = crates_package_paths(&files).expect("paths");
        let args_log = scratch.path().join("args");
        let exported = scratch.path().join("exported");
        let fake = scratch.path().join("cargo-semver-checks");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"$@\" > {}\nwhile [ $# -gt 0 ]; do if [ \"$1\" = --baseline-root ]; then cp \"$2/Cargo.toml\" {}; fi; shift; done\necho '--- failure pub_function_missing: pub fn removed or renamed ---'\nexit 1\n",
                args_log.display(),
                exported.display()
            ),
        )
        .expect("fake tool");
        set_mode(&fake, 0o755);
        let repo_dir = repo.display().to_string();
        let run_in = |dir: &str, version: &str, mode: ApiCompatMode| {
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
        let run = |version: &str, mode: ApiCompatMode| run_in("pkg", version, mode);
        let refused = run("1.2.1", ApiCompatMode::Fail).expect_err("patch bump refused");
        assert!(refused.contains("1.2.0 -> 1.2.1"), "{refused}");
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(logged.contains("--baseline-root"), "{logged}");
        assert!(!logged.contains("--baseline-rev"), "{logged}");
        assert_eq!(
            std::fs::read_to_string(&exported).expect("exported manifest"),
            "[package]\nname = \"pkg\"\n"
        );
        assert!(run("2.0.0", ApiCompatMode::Fail).is_ok());
        assert!(run("1.2.1", ApiCompatMode::Warn).is_ok());
        std::fs::remove_file(&args_log).expect("clear log");
        assert!(run("1.2.1", ApiCompatMode::Off).is_ok());
        assert!(!args_log.exists(), "off must not run the tool");
        assert!(run_in("moved", "2.0.0", ApiCompatMode::Fail).is_ok());
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
        write_crate_file(&repo, "pkg/Cargo.toml", manifest);
        write_crate_file(&repo, "pkg/src/lib.rs", "pub fn hello() {}\n");
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "-m",
                "x",
            ],
            vec!["tag", "v1.2.0"],
        ] {
            dx_process::capture::run_checked(
                dx_process::capture::git(&repo).args(&args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }
        let current = scratch.path().join("in/pkg");
        let files = vec![
            write_crate_file(&current, "Cargo.toml", manifest),
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
                    (ApiCompatMode::Fail, Baseline::Git),
                    "pkg",
                )
            })
        };
        let refused = run("1.2.1").expect_err("patch bump refused");
        assert!(refused.contains("1.2.0 -> 1.2.1"), "{refused}");
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
}
