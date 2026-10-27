use std::io;
use std::path::{Path, PathBuf};

use pgp::composed::{
    ArmorOptions, Deserializable, DetachedSignature, SignedPublicKey, SignedSecretKey,
};
use pgp::crypto::hash::HashAlgorithm;
use pgp::types::Password;

use crate::http;
use crate::run::{env_or_empty, OwnerGate};
use crate::{copy_verified, deploy_profile, tempfile_stage_dir, zip_archive, DeployRun};

const MAVEN_CHECKSUMS: [&str; 4] = ["md5", "sha1", "sha256", "sha512"];

/// Maps one Maven group to a path.
pub fn maven_group_path(group: &str) -> String {
    group.replace('.', "/")
}

/// Rejects Maven coordinates that are not safe single path components.
pub fn maven_coordinates_check(group: &str, artifact: &str, version: &str) -> io::Result<()> {
    let bad = |message: String| Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    if group.is_empty() {
        return bad("maven file repo: need a non-empty groupId".to_owned());
    }
    if artifact.is_empty() {
        return bad("maven file repo: need a non-empty artifactId".to_owned());
    }
    if version.is_empty() {
        return bad("maven file repo: need a non-empty version".to_owned());
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if !group.chars().all(plain) || group.split('.').any(str::is_empty) {
        return bad(format!(
            "maven file repo: invalid groupId '{group}': want non-empty [A-Za-z0-9_-] segments separated by single dots"
        ));
    }
    if !artifact.chars().all(plain) || artifact.chars().all(|c| c == '.') {
        return bad(format!(
            "maven file repo: invalid artifactId '{artifact}': want [A-Za-z0-9._-] that is not only dots"
        ));
    }
    if !version.chars().all(|c| plain(c) || c == '+') || version.chars().all(|c| c == '.') {
        return bad(format!(
            "maven file repo: invalid version '{version}': want [A-Za-z0-9._+-] that is not only dots"
        ));
    }
    Ok(())
}

fn maven_reject_symlinks(
    outdir: &Path,
    rel: &Path,
    artifact: &str,
    version: &str,
) -> io::Result<()> {
    let mut current = outdir.to_path_buf();
    let check = |path: &Path| -> io::Result<()> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "maven file repo: {artifact}:{version}: refusing symlink at {}",
                    path.display()
                ),
            )),
            _ => Ok(()),
        }
    };
    for component in rel.components() {
        current.push(component);
        check(&current)?;
    }
    for base in [
        format!("{artifact}-{version}.jar"),
        format!("{artifact}-{version}.pom"),
    ] {
        check(&current.join(&base))?;
        for suffix in MAVEN_CHECKSUMS.into_iter().chain(["asc"]) {
            check(&current.join(format!("{base}.{suffix}")))?;
        }
    }
    check(
        &current
            .parent()
            .unwrap_or(&current)
            .join("maven-metadata.xml"),
    )
}

/// Returns the lowercase hex md5, sha1, sha256 and sha512 of some bytes.
pub fn maven_checksums(bytes: &[u8]) -> [(&'static str, String); 4] {
    use sha2::Digest as _;
    [
        (MAVEN_CHECKSUMS[0], format!("{:x}", md5::Md5::digest(bytes))),
        (
            MAVEN_CHECKSUMS[1],
            format!("{:x}", sha1::Sha1::digest(bytes)),
        ),
        (
            MAVEN_CHECKSUMS[2],
            format!("{:x}", sha2::Sha256::digest(bytes)),
        ),
        (
            MAVEN_CHECKSUMS[3],
            format!("{:x}", sha2::Sha512::digest(bytes)),
        ),
    ]
}

/// Builds one Maven file repo.
pub fn maven_build_file_repo(
    jar_src: &Path,
    pom_src: &Path,
    outdir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
    signer: Option<&MavenSigner>,
) -> io::Result<PathBuf> {
    maven_coordinates_check(group, artifact, version)?;
    let rel = PathBuf::from(artifact.to_owned() + "-repo")
        .join(maven_group_path(group))
        .join(artifact)
        .join(version);
    maven_reject_symlinks(outdir, &rel, artifact, version)?;
    let pom_bytes = std::fs::read(pom_src)?;
    if let Some(error) = maven_pom_metadata_error(&pom_bytes, group, artifact, version) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("maven file repo: {artifact}:{version}: {error}"),
        ));
    }
    let house = outdir.join(artifact.to_owned() + "-repo");
    let dest_dir = house
        .join(maven_group_path(group))
        .join(artifact)
        .join(version);
    std::fs::create_dir_all(&dest_dir)?;
    let jar_base = format!("{artifact}-{version}.jar");
    let pom_base = format!("{artifact}-{version}.pom");
    for (src, base) in [(jar_src, jar_base), (pom_src, pom_base)] {
        let dest = dest_dir.join(&base);
        copy_verified(src, &dest)?;
        let bytes = std::fs::read(&dest)?;
        for (suffix, digest) in maven_checksums(&bytes) {
            std::fs::write(dest_dir.join(format!("{base}.{suffix}")), digest.as_bytes())?;
        }
        let asc = dest_dir.join(format!("{base}.asc"));
        match signer {
            Some(signer) => {
                maven_sign_detached(signer, &dest)?;
            }
            None if asc.is_file() => std::fs::remove_file(&asc)?,
            None => {}
        }
    }
    let metadata_dir = house.join(maven_group_path(group)).join(artifact);
    std::fs::create_dir_all(&metadata_dir)?;
    let metadata = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<metadata>\n  <groupId>{group}</groupId>\n  <artifactId>{artifact}</artifactId>\n  <version>{version}</version>\n  <versioning>\n    <latest>{version}</latest>\n    <release>{version}</release>\n    <versions>\n      <version>{version}</version>\n    </versions>\n  </versioning>\n</metadata>\n"
    );
    std::fs::write(metadata_dir.join("maven-metadata.xml"), metadata.as_bytes())?;
    Ok(house)
}

/// Reads the text of every element path in one POM.
fn maven_pom_fields(pom: &[u8]) -> Result<std::collections::HashMap<String, String>, String> {
    use quick_xml::events::Event;
    let xml = std::str::from_utf8(pom).map_err(|_| "pom is not UTF-8".to_owned())?;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut fields = std::collections::HashMap::new();
    let mut root: Option<String> = None;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("pom is not valid XML: {error}"))?;
        match event {
            Event::Start(start) => {
                let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
                root.get_or_insert_with(|| name.clone());
                stack.push(name);
                fields.entry(stack.join("/")).or_insert_with(String::new);
            }
            Event::Empty(start) => {
                let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
                root.get_or_insert_with(|| name.clone());
                stack.push(name);
                fields.entry(stack.join("/")).or_insert_with(String::new);
                stack.pop();
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(text) => {
                let text = text
                    .xml10_content()
                    .map_err(|error| format!("pom text is not valid: {error}"))?;
                if let Some(value) = fields.get_mut(&stack.join("/")) {
                    value.push_str(&text);
                }
            }
            Event::CData(text) => {
                let text = text
                    .decode()
                    .map_err(|error| format!("pom text is not valid: {error}"))?;
                if let Some(value) = fields.get_mut(&stack.join("/")) {
                    value.push_str(&text);
                }
            }
            Event::GeneralRef(reference) => {
                let resolved = match reference
                    .resolve_char_ref()
                    .map_err(|error| format!("pom text is not valid: {error}"))?
                {
                    Some(ch) => ch.to_string(),
                    None => {
                        let name = reference
                            .decode()
                            .map_err(|error| format!("pom text is not valid: {error}"))?;
                        quick_xml::escape::resolve_predefined_entity(&name)
                            .ok_or_else(|| format!("pom uses unknown entity &{name};"))?
                            .to_owned()
                    }
                };
                if let Some(value) = fields.get_mut(&stack.join("/")) {
                    value.push_str(&resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    match root.as_deref() {
        Some("project") => Ok(fields),
        Some(other) => Err(format!("pom root is <{other}>, want <project>")),
        None => Err("pom has no <project> element".to_owned()),
    }
}

/// Returns why one POM does not meet the Maven Central requirements, or None.
pub fn maven_pom_metadata_error(
    pom: &[u8],
    group: &str,
    artifact: &str,
    version: &str,
) -> Option<String> {
    let fields = match maven_pom_fields(pom) {
        Ok(fields) => fields,
        Err(error) => return Some(error),
    };
    let text = |path: &str| {
        fields
            .get(&format!("project/{path}"))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let mut problems = Vec::new();
    let mut missing = Vec::new();
    let found_group = text("groupId").or_else(|| text("parent/groupId"));
    for (field, found, want) in [
        ("groupId", found_group, group),
        ("artifactId", text("artifactId"), artifact),
        ("version", text("version"), version),
    ] {
        match found {
            None => missing.push(field),
            Some(found) if found != want => {
                problems.push(format!("{field} is '{found}', want '{want}'"));
            }
            Some(_) => {}
        }
    }
    if let Some(packaging) = text("packaging") {
        if packaging != "jar" {
            problems.push(format!("packaging is '{packaging}', want jar"));
        }
    }
    for field in [
        "name",
        "description",
        "url",
        "licenses/license/name",
        "scm/url",
        "scm/connection",
        "scm/developerConnection",
    ] {
        if text(field).is_none() {
            missing.push(field);
        }
    }
    if !fields.contains_key("project/developers/developer") {
        missing.push("developers/developer");
    }
    if !missing.is_empty() {
        problems.push(format!("missing {}", missing.join(", ")));
    }
    if problems.is_empty() {
        None
    } else {
        Some(format!("pom is not Central-ready: {}", problems.join("; ")))
    }
}

/// Secret signing key material for Maven detached signatures.
pub struct MavenSigner {
    key: MavenSigningKey,
}

/// The unlocked key that makes signatures.
enum MavenSigningKey {
    Primary(pgp::packet::SecretKey),
    Subkey(pgp::packet::SecretSubkey),
}

impl MavenSigner {
    /// Parses an armored secret key and unlocks its signing key with the passphrase.
    pub fn from_armored(key: &str, passphrase: &str) -> Result<Self, String> {
        let (secret, _) = SignedSecretKey::from_armor_single(key.as_bytes()).map_err(|_| {
            "maven_deploy: MAVEN_GPG_KEY is not an armored OpenPGP secret key; refusing".to_owned()
        })?;
        let password = Password::from(passphrase);
        let subkey = secret
            .secret_subkeys
            .into_iter()
            .find(|sub| sub.signatures.iter().any(|sig| sig.key_flags().sign()));
        let mut key = match subkey {
            Some(sub) => MavenSigningKey::Subkey(sub.key),
            None => MavenSigningKey::Primary(secret.primary_key),
        };
        let unlocked = match &mut key {
            MavenSigningKey::Primary(primary) => primary.remove_password(&password),
            MavenSigningKey::Subkey(sub) => sub.remove_password(&password),
        };
        unlocked.map_err(|_| {
            "maven_deploy: MAVEN_GPG_PASSPHRASE does not unlock MAVEN_GPG_KEY; refusing".to_owned()
        })?;
        Ok(Self { key })
    }
}

/// Writes an armored detached SHA-256 signature next to one file.
pub fn maven_sign_detached(signer: &MavenSigner, path: &Path) -> io::Result<PathBuf> {
    let data = std::fs::read(path)?;
    let fail = |_| io::Error::other(format!("maven signing: cannot sign {}", path.display()));
    let rng = rand::thread_rng();
    let empty = Password::empty();
    let signature = match &signer.key {
        MavenSigningKey::Primary(key) => {
            DetachedSignature::sign_binary_data(rng, key, &empty, HashAlgorithm::Sha256, &data[..])
        }
        MavenSigningKey::Subkey(key) => {
            DetachedSignature::sign_binary_data(rng, key, &empty, HashAlgorithm::Sha256, &data[..])
        }
    }
    .map_err(fail)?;
    let armored = signature
        .to_armored_string(ArmorOptions::default())
        .map_err(fail)?;
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".asc");
    let out = path.with_file_name(name);
    std::fs::write(&out, armored.as_bytes())?;
    Ok(out)
}

/// Checks one armored detached signature against an armored public key.
pub fn maven_verify_detached(public_armored: &str, data: &[u8], asc: &str) -> Result<(), String> {
    let (public, _) = SignedPublicKey::from_armor_single(public_armored.as_bytes())
        .map_err(|error| format!("maven verify: public key is not valid: {error}"))?;
    let (signature, _) = DetachedSignature::from_armor_single(asc.as_bytes())
        .map_err(|error| format!("maven verify: signature is not valid: {error}"))?;
    if signature.verify(&public, data).is_ok()
        || public
            .public_subkeys
            .iter()
            .any(|sub| signature.verify(sub, data).is_ok())
    {
        Ok(())
    } else {
        Err("maven verify: signature does not match the data and key".to_owned())
    }
}

/// Central Portal Publisher API base URL.
pub const MAVEN_DEFAULT_REPOSITORY_URL: &str = "https://central.sonatype.com/api/v1/publisher/";

/// Hosts that serve the Central Portal Publisher API.
pub const MAVEN_SUPPORTED_HOSTS: [&str; 1] = ["central.sonatype.com"];

const MAVEN_PUBLISHER_PATH: &str = "/api/v1/publisher/";

/// Returns the retired OSSRH host named by a repository URL.
pub fn maven_retired_host(repository_url: &str) -> Option<&'static str> {
    let rest = repository_url.strip_prefix("https://")?;
    let host = rest.split('/').next().unwrap_or("");
    ["oss.sonatype.org", "s01.oss.sonatype.org"]
        .into_iter()
        .find(|retired| host.eq_ignore_ascii_case(retired))
}

/// Returns why one repository URL is not the Central Portal Publisher API, or None.
pub fn maven_repository_url_error(repository_url: &str) -> Option<String> {
    let Some(rest) = repository_url.strip_prefix("https://") else {
        return Some(format!(
            "maven_deploy: invalid repository_url '{repository_url}': want an https URL so uploads never go over plaintext"
        ));
    };
    if let Some(host) = maven_retired_host(repository_url) {
        return Some(format!(
            "maven_deploy: invalid repository_url '{repository_url}': {host} is retired OSSRH infrastructure (use '{MAVEN_DEFAULT_REPOSITORY_URL}')"
        ));
    }
    let host = rest.split('/').next().unwrap_or("");
    let supported = MAVEN_SUPPORTED_HOSTS
        .iter()
        .any(|supported| host.eq_ignore_ascii_case(supported));
    if !supported || &rest[host.len()..] != MAVEN_PUBLISHER_PATH {
        return Some(format!(
            "maven_deploy: invalid repository_url '{repository_url}': want the Central Portal Publisher API '{MAVEN_DEFAULT_REPOSITORY_URL}'"
        ));
    }
    None
}

fn maven_bundle_walk(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = rel.to_owned() + &name;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            maven_bundle_walk(&entry.path(), &(path + "/"), out)?;
        } else if kind.is_file() && name != "maven-metadata.xml" {
            out.push((path, std::fs::read(entry.path())?));
        }
    }
    Ok(())
}

/// Zips one staged Maven file repo as a Central Portal bundle, without maven-metadata.xml.
pub fn maven_bundle_zip(house: &Path) -> io::Result<Vec<u8>> {
    let mut files = Vec::new();
    maven_bundle_walk(house, "", &mut files)?;
    files.sort();
    let members: Vec<(&str, &[u8], u32)> = files
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice(), 0o644))
        .collect();
    zip_archive(&members)
}

/// Refusal for a Portal upload rejected as unauthorized.
pub const MAVEN_BAD_CREDENTIALS: &str = "maven_deploy: the Central Portal rejected MAVEN_USERNAME/MAVEN_PASSWORD; refusing (set them to a Portal user token from https://central.sonatype.com/usertoken)";

/// Uploads one bundle zip to the Publisher API as a USER_MANAGED deployment and returns its id.
pub fn maven_upload_bundle(
    agent: &ureq::Agent,
    base_url: &str,
    bearer: &str,
    name: &str,
    zip: &[u8],
) -> Result<String, String> {
    use std::io::Read as _;
    let url = format!(
        "{base_url}upload?publishingType=USER_MANAGED&name={}",
        http::query_escape(name)
    );
    let (content_type, body) = http::multipart_single_file("bundle", "bundle.zip", zip);
    let mut response = agent
        .post(&url)
        .header("Authorization", bearer)
        .header("Content-Type", &content_type)
        .send(&body[..])
        .map_err(|error| format!("maven_deploy: upload to {url} failed: {error}"))?;
    let status = response.status().as_u16();
    let mut text = Vec::new();
    let read = response
        .body_mut()
        .as_reader()
        .take(2048)
        .read_to_end(&mut text);
    let text = String::from_utf8_lossy(&text).trim().to_owned();
    match status {
        201 => match read {
            Ok(_) if !text.is_empty() => Ok(text),
            _ => Err(format!(
                "maven_deploy: upload to {url} returned 201 without a deployment id"
            )),
        },
        401 | 403 => Err(MAVEN_BAD_CREDENTIALS.to_owned()),
        _ => Err(format!(
            "maven_deploy: upload to {url} failed with HTTP {status}: {text}"
        )),
    }
}

/// Owner approval gate for maven_deploy.
pub(crate) const MAVEN_GATE: OwnerGate = OwnerGate {
    tool: "maven_deploy",
    action: "staging",
    credentials: &["MAVEN_USERNAME", "MAVEN_PASSWORD"],
    approved: "MAVEN_PUBLISH_APPROVED",
    live: "MAVEN_PUBLISH_LIVE",
};

/// Credentials and signing state for one live Maven upload.
pub struct MavenLive {
    /// Repository username.
    pub username: String,
    /// Repository password or token.
    pub password: String,
    /// Signer parsed from MAVEN_GPG_KEY.
    pub signer: Option<MavenSigner>,
    /// Whether the upload carries no signature.
    pub unsigned: bool,
}

/// Checks every live-upload refusal and returns the upload settings.
pub fn maven_live_gate(
    env: &dyn Fn(&str) -> String,
    version: &str,
    repository_url: &str,
) -> Result<MavenLive, String> {
    if version == "0.0.0" {
        return Err(
            "maven_deploy: live staging refuses version 0.0.0; set a real version".to_owned(),
        );
    }
    if version.contains("-SNAPSHOT") {
        return Err(
            "maven_deploy: live staging refuses -SNAPSHOT versions; set a release version"
                .to_owned(),
        );
    }
    let secrets = MAVEN_GATE.authorize(env)?;
    if let Some(error) = maven_repository_url_error(repository_url) {
        return Err(error);
    }
    let passphrase = env("MAVEN_GPG_PASSPHRASE");
    let key = env("MAVEN_GPG_KEY");
    let signer = if key.is_empty() {
        None
    } else {
        Some(MavenSigner::from_armored(&key, &passphrase)?)
    };
    if key.is_empty() && !passphrase.is_empty() {
        return Err(
            "maven staging: MAVEN_GPG_PASSPHRASE is set without MAVEN_GPG_KEY; refusing (set MAVEN_GPG_KEY, or unset the passphrase and record unsigned staging with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)".to_owned(),
        );
    }
    let unsigned = signer.is_none();
    if unsigned && env("MAVEN_ALLOW_UNSIGNED") != "1" {
        return Err(
            "maven staging: no MAVEN_GPG_KEY; refusing unsigned publish (set MAVEN_GPG_KEY and MAVEN_GPG_PASSPHRASE, or record unsigned staging explicitly with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)".to_owned(),
        );
    }
    Ok(MavenLive {
        username: secrets.get("MAVEN_USERNAME").to_owned(),
        password: secrets.get("MAVEN_PASSWORD").to_owned(),
        signer,
        unsigned,
    })
}

/// Runs the Maven deploy launcher.
pub fn maven_main(
    jar_rloc: &str,
    pom_rloc: &str,
    group: &str,
    artifact: &str,
    version: &str,
    repository_url: &str,
    argv: &[String],
) -> i32 {
    if argv.len() > 2 {
        eprintln!(
            "maven_deploy: this deploy target takes at most an output directory; the file repo is exactly the files pinned at analysis time"
        );
        return 1;
    }
    if let Err(error) = maven_coordinates_check(group, artifact, version) {
        eprintln!("maven_deploy: {error}");
        return 1;
    }
    let Some(run) = DeployRun::start(
        "maven_deploy",
        "the file repo is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(jar) = run.resolve(jar_rloc) else {
        return 1;
    };
    let Some(pom) = run.resolve(pom_rloc) else {
        return 1;
    };
    if MAVEN_GATE.requested(env_or_empty) {
        let live = match maven_live_gate(&env_or_empty, version, repository_url) {
            Ok(live) => live,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let workdir = match tempfile_stage_dir("maven-deploy-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        };
        let bundle = maven_build_file_repo(
            &jar,
            &pom,
            &workdir,
            group,
            artifact,
            version,
            live.signer.as_ref(),
        )
        .and_then(|house| maven_bundle_zip(&house));
        let _ = std::fs::remove_dir_all(&workdir);
        let bundle = match bundle {
            Ok(bundle) => bundle,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        };
        if live.unsigned {
            println!(
                "maven_deploy: unsigned staging recorded (MAVEN_ALLOW_UNSIGNED=1 with owner approval; no GPG signature attached)"
            );
            println!("maven_deploy: Central Portal validation rejects unsigned bundles");
        }
        let name = format!("{group}:{artifact}:{version}");
        match maven_upload_bundle(
            &http::publish_agent(),
            repository_url,
            &http::bearer_basic(&live.username, &live.password),
            &name,
            &bundle,
        ) {
            Ok(id) => {
                println!("maven_deploy: uploaded {name} to {repository_url} as deployment {id}");
                println!(
                    "maven_deploy: not published yet; validate and publish the deployment in the Central Portal"
                );
                println!("  https://central.sonatype.com/publishing/deployments");
                return 0;
            }
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    }
    let key = env_or_empty("MAVEN_GPG_KEY");
    let signer = if key.is_empty() {
        None
    } else {
        match MavenSigner::from_armored(&key, &env_or_empty("MAVEN_GPG_PASSPHRASE")) {
            Ok(signer) => Some(signer),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    };
    match maven_build_file_repo(
        &jar,
        &pom,
        outdir,
        group,
        artifact,
        version,
        signer.as_ref(),
    ) {
        Ok(house) => {
            let profile = deploy_profile();
            println!("maven_deploy: staged {group}:{artifact}:{version} (profile: {profile})");
            println!("  repo: {}", house.display());
            println!("  url: file://{}", house.display());
            println!("  signed: {}", if signer.is_some() { "yes" } else { "no" });
            0
        }
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::resolve_runfile;
    use crate::test_support::{env_test_in_child, scratch_dir, with_env};
    use pgp::composed::{EncryptionCaps, KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};

    const PASSPHRASE: &str = "maven-test-passphrase";

    pub(crate) fn central_pom(group: &str, artifact: &str, version: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>{group}</groupId>\n  <artifactId>{artifact}</artifactId>\n  <version>{version}</version>\n  <packaging>jar</packaging>\n  <name>{artifact}</name>\n  <description>Test &amp; demo.</description>\n  <url>https://example.com/{artifact}</url>\n  <licenses><license><name>MIT</name></license></licenses>\n  <developers><developer><name>dx</name></developer></developers>\n  <scm>\n    <connection>scm:git:https://example.com/{artifact}.git</connection>\n    <developerConnection>scm:git:ssh://git@example.com/{artifact}.git</developerConnection>\n    <url>https://example.com/{artifact}</url>\n  </scm>\n</project>\n"
        )
    }

    fn test_key(signing_subkey: bool) -> (String, String) {
        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(!signing_subkey)
            .can_encrypt(EncryptionCaps::None)
            .passphrase(Some(PASSPHRASE.to_owned()))
            .primary_user_id("dx maven test <maven-test@example.invalid>".into());
        if signing_subkey {
            let mut sub = SubkeyParamsBuilder::default();
            sub.key_type(KeyType::Ed25519Legacy)
                .can_sign(true)
                .can_encrypt(EncryptionCaps::None)
                .passphrase(Some(PASSPHRASE.to_owned()));
            params.subkeys(vec![sub.build().expect("subkey params")]);
        }
        let secret = params
            .build()
            .expect("key params")
            .generate(rand::thread_rng())
            .expect("generate key");
        let public = secret.to_public_key();
        (
            secret
                .to_armored_string(ArmorOptions::default())
                .expect("armor secret"),
            public
                .to_armored_string(ArmorOptions::default())
                .expect("armor public"),
        )
    }

    fn runfile(var: &str) -> PathBuf {
        resolve_runfile(&std::env::var(var).expect(var)).expect("runfile")
    }

    fn gate_env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |key: &str| {
            vars.iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
                .unwrap_or_default()
        }
    }

    const APPROVED: [(&str, &str); 3] = [
        ("MAVEN_USERNAME", "user"),
        ("MAVEN_PASSWORD", "pass"),
        ("MAVEN_PUBLISH_APPROVED", "1"),
    ];

    fn gate_error(vars: &[(&str, &str)], version: &str, url: &str) -> Option<String> {
        maven_live_gate(&gate_env(vars), version, url).err()
    }

    #[test]
    fn maven_checksums_match_known_vectors() {
        let sums = maven_checksums(b"abc");
        assert_eq!(
            sums,
            [
                ("md5", "900150983cd24fb0d6963f7d28e17f72".to_owned()),
                ("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d".to_owned()),
                (
                    "sha256",
                    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()
                ),
                (
                    "sha512",
                    "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn maven_bundle_writes_checksums_and_signatures() {
        let scratch = scratch_dir();
        let jar = runfile("MAVEN_DEMO_JAR");
        let pom = runfile("MAVEN_DEMO_POM");
        let jar_bytes = std::fs::read(&jar).expect("read jar");
        assert!(jar_bytes.starts_with(b"PK\x03\x04"), "jar is not a zip");
        let (secret, public) = test_key(true);
        let signer = MavenSigner::from_armored(&secret, PASSPHRASE).expect("signer");
        let house = maven_build_file_repo(
            &jar,
            &pom,
            scratch.path(),
            "com.example",
            "maven_demo",
            "0.0.0",
            Some(&signer),
        )
        .expect("repo");
        let dir = house.join("com/example/maven_demo/0.0.0");
        for (base, src) in [
            ("maven_demo-0.0.0.jar", &jar),
            ("maven_demo-0.0.0.pom", &pom),
        ] {
            let staged = std::fs::read(dir.join(base)).expect("staged");
            assert_eq!(staged, std::fs::read(src).expect("source"), "{base}");
            for (suffix, digest) in maven_checksums(&staged) {
                let written =
                    std::fs::read_to_string(dir.join(format!("{base}.{suffix}"))).expect("sum");
                assert_eq!(written, digest, "{base}.{suffix}");
                assert!(written
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            }
            let asc = std::fs::read_to_string(dir.join(format!("{base}.asc"))).expect("asc");
            assert!(asc.starts_with("-----BEGIN PGP SIGNATURE-----"), "{asc}");
            maven_verify_detached(&public, &staged, &asc).expect("signature verifies");
            let mut tampered = staged.clone();
            tampered[0] ^= 1;
            assert!(maven_verify_detached(&public, &tampered, &asc).is_err());
        }
        let unsigned = maven_build_file_repo(
            &jar,
            &pom,
            scratch.path(),
            "com.example",
            "maven_demo",
            "0.0.0",
            None,
        )
        .expect("unsigned repo");
        assert!(!unsigned
            .join("com/example/maven_demo/0.0.0/maven_demo-0.0.0.jar.asc")
            .exists());
    }

    #[test]
    fn maven_verify_accepts_gnupg_fixture() {
        let public = std::fs::read_to_string(runfile("MAVEN_GNUPG_PUBKEY")).expect("pubkey");
        let payload = std::fs::read(runfile("MAVEN_GNUPG_PAYLOAD")).expect("payload");
        let asc = std::fs::read_to_string(runfile("MAVEN_GNUPG_SIGNATURE")).expect("asc");
        maven_verify_detached(&public, &payload, &asc).expect("gnupg signature verifies");
        let mut flipped = payload.clone();
        flipped[0] ^= 1;
        assert_eq!(
            maven_verify_detached(&public, &flipped, &asc),
            Err("maven verify: signature does not match the data and key".to_owned())
        );
    }

    #[test]
    fn maven_pom_metadata_error_reports_missing_central_fields() {
        let good = central_pom("com.example", "demo", "1.0.0");
        assert_eq!(
            maven_pom_metadata_error(good.as_bytes(), "com.example", "demo", "1.0.0"),
            None
        );
        let fixture = std::fs::read(runfile("MAVEN_DEMO_POM")).expect("pom fixture");
        assert_eq!(
            maven_pom_metadata_error(&fixture, "com.example", "maven_demo", "0.0.0"),
            None
        );
        let parent = good.replace(
            "<groupId>com.example</groupId>",
            "<parent><groupId>com.example</groupId></parent>",
        );
        assert_eq!(
            maven_pom_metadata_error(parent.as_bytes(), "com.example", "demo", "1.0.0"),
            None
        );
        for (from, field) in [
            ("<groupId>com.example</groupId>", "groupId"),
            ("<artifactId>demo</artifactId>", "artifactId"),
            ("<version>1.0.0</version>", "version"),
            ("<name>demo</name>", "name"),
            ("<description>Test &amp; demo.</description>", "description"),
            ("<url>https://example.com/demo</url>\n  <licenses>", "url"),
            ("<name>MIT</name>", "licenses/license/name"),
            (
                "<developer><name>dx</name></developer>",
                "developers/developer",
            ),
            ("<url>https://example.com/demo</url>\n  </scm>", "scm/url"),
            (
                "<connection>scm:git:https://example.com/demo.git</connection>",
                "scm/connection",
            ),
            (
                "<developerConnection>scm:git:ssh://git@example.com/demo.git</developerConnection>",
                "scm/developerConnection",
            ),
        ] {
            assert!(good.contains(from), "{from}");
            let to = if from.ends_with("<licenses>") {
                "<licenses>"
            } else if from.ends_with("</scm>") {
                "</scm>"
            } else {
                ""
            };
            let pom = good.replacen(from, to, 1);
            assert_eq!(
                maven_pom_metadata_error(pom.as_bytes(), "com.example", "demo", "1.0.0"),
                Some(format!("pom is not Central-ready: missing {field}")),
                "{field}"
            );
        }
        assert_eq!(
            maven_pom_metadata_error(good.as_bytes(), "com.other", "demo", "2.0.0"),
            Some(
                "pom is not Central-ready: groupId is 'com.example', want 'com.other'; version is '1.0.0', want '2.0.0'"
                    .to_owned()
            )
        );
        let war = good.replace("<packaging>jar</packaging>", "<packaging>war</packaging>");
        assert_eq!(
            maven_pom_metadata_error(war.as_bytes(), "com.example", "demo", "1.0.0"),
            Some("pom is not Central-ready: packaging is 'war', want jar".to_owned())
        );
        assert_eq!(
            maven_pom_metadata_error(b"<project/>", "g", "a", "1"),
            Some(
                "pom is not Central-ready: missing groupId, artifactId, version, name, description, url, licenses/license/name, scm/url, scm/connection, scm/developerConnection, developers/developer"
                    .to_owned()
            )
        );
        assert_eq!(
            maven_pom_metadata_error(b"<settings/>", "g", "a", "1"),
            Some("pom root is <settings>, want <project>".to_owned())
        );
        assert!(maven_pom_metadata_error(b"<project><name>", "g", "a", "1").is_some());
    }

    #[test]
    fn maven_gate_rejects_version_zero() {
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_GPG_PASSPHRASE", "pw"));
        assert_eq!(
            gate_error(&vars, "0.0.0", MAVEN_DEFAULT_REPOSITORY_URL),
            Some("maven_deploy: live staging refuses version 0.0.0; set a real version".to_owned())
        );
    }

    #[test]
    fn maven_gate_rejects_snapshot_version() {
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_GPG_PASSPHRASE", "pw"));
        for version in ["1.0.0-SNAPSHOT", "1.0-SNAPSHOT+b5"] {
            assert_eq!(
                gate_error(&vars, version, MAVEN_DEFAULT_REPOSITORY_URL),
                Some(
                    "maven_deploy: live staging refuses -SNAPSHOT versions; set a release version"
                        .to_owned()
                )
            );
        }
    }

    #[test]
    fn maven_gate_rejects_missing_credentials() {
        for vars in [
            &[("MAVEN_PASSWORD", "pass"), ("MAVEN_PUBLISH_APPROVED", "1")][..],
            &[("MAVEN_USERNAME", "user"), ("MAVEN_PUBLISH_APPROVED", "1")][..],
        ] {
            assert_eq!(
                gate_error(vars, "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL),
                Some("maven_deploy: live staging needs MAVEN_USERNAME plus MAVEN_PASSWORD plus explicit owner approval (MAVEN_PUBLISH_APPROVED=1); refusing".to_owned())
            );
        }
    }

    #[test]
    fn maven_gate_rejects_missing_approval() {
        assert_eq!(
            gate_error(&APPROVED[..2], "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL),
            Some(
                "maven_deploy: live staging needs MAVEN_PUBLISH_APPROVED=1 (owner approval); refusing"
                    .to_owned()
            )
        );
    }

    #[test]
    fn maven_gate_rejects_unsigned_without_allow_and_accepts_with_allow() {
        assert_eq!(
            gate_error(&APPROVED, "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL),
            Some("maven staging: no MAVEN_GPG_KEY; refusing unsigned publish (set MAVEN_GPG_KEY and MAVEN_GPG_PASSPHRASE, or record unsigned staging explicitly with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)".to_owned())
        );
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        let live = maven_live_gate(&gate_env(&vars), "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL)
            .ok()
            .expect("unsigned allowed");
        assert!(live.unsigned);
        assert!(live.signer.is_none());
        assert_eq!(
            (live.username.as_str(), live.password.as_str()),
            ("user", "pass")
        );
        let (secret, _) = test_key(false);
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_GPG_KEY", &secret));
        vars.push(("MAVEN_GPG_PASSPHRASE", PASSPHRASE));
        let live = maven_live_gate(&gate_env(&vars), "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL)
            .ok()
            .expect("signed");
        assert!(!live.unsigned);
        assert!(live.signer.is_some());
    }

    #[test]
    fn maven_gate_rejects_passphrase_without_key() {
        for allow in [false, true] {
            let mut vars = APPROVED.to_vec();
            vars.push(("MAVEN_GPG_PASSPHRASE", PASSPHRASE));
            if allow {
                vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
            }
            let error = gate_error(&vars, "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL).expect("refused");
            assert!(error.contains("without MAVEN_GPG_KEY"));
            assert!(!error.contains(PASSPHRASE));
        }
    }

    #[test]
    fn maven_gate_rejects_bad_key_or_passphrase() {
        let (secret, public) = test_key(true);
        for (key, passphrase, want) in [
            (
                "not a key",
                PASSPHRASE,
                "maven_deploy: MAVEN_GPG_KEY is not an armored OpenPGP secret key; refusing",
            ),
            (
                public.as_str(),
                PASSPHRASE,
                "maven_deploy: MAVEN_GPG_KEY is not an armored OpenPGP secret key; refusing",
            ),
            (
                secret.as_str(),
                "wrong-passphrase",
                "maven_deploy: MAVEN_GPG_PASSPHRASE does not unlock MAVEN_GPG_KEY; refusing",
            ),
            (
                secret.as_str(),
                "",
                "maven_deploy: MAVEN_GPG_PASSPHRASE does not unlock MAVEN_GPG_KEY; refusing",
            ),
        ] {
            let mut vars = APPROVED.to_vec();
            vars.push(("MAVEN_GPG_KEY", key));
            vars.push(("MAVEN_GPG_PASSPHRASE", passphrase));
            let error = gate_error(&vars, "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL).expect("refused");
            assert_eq!(error, want);
            assert!(!error.contains(PASSPHRASE) && !error.contains("wrong-passphrase"));
            assert!(!error.contains("PGP PRIVATE KEY"));
        }
    }

    #[test]
    fn maven_gate_rejects_retired_host_at_runtime() {
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        for (url, host) in [
            (
                "https://oss.sonatype.org/service/local/staging/deploy/maven2/",
                "oss.sonatype.org",
            ),
            (
                "https://S01.oss.sonatype.org/content/",
                "s01.oss.sonatype.org",
            ),
        ] {
            assert_eq!(
                gate_error(&vars, "1.0.0", url),
                Some(format!("maven_deploy: invalid repository_url '{url}': {host} is retired OSSRH infrastructure (use '{MAVEN_DEFAULT_REPOSITORY_URL}')"))
            );
        }
        assert_eq!(
            gate_error(&vars, "1.0.0", MAVEN_DEFAULT_REPOSITORY_URL),
            None
        );
    }

    #[test]
    fn maven_gate_rejects_unsupported_host_at_runtime() {
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        for url in [
            "https://repo.example.com/maven2/",
            "https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/",
            "https://central.sonatype.com/api/v1/",
            "https://central.sonatype.com/api/v1/publisher/upload",
        ] {
            assert_eq!(
                gate_error(&vars, "1.0.0", url),
                Some(format!("maven_deploy: invalid repository_url '{url}': want the Central Portal Publisher API '{MAVEN_DEFAULT_REPOSITORY_URL}'"))
            );
        }
        assert_eq!(
            gate_error(
                &vars,
                "1.0.0",
                "https://Central.Sonatype.com/api/v1/publisher/"
            ),
            None
        );
    }

    #[test]
    fn maven_gate_rejects_plaintext_url() {
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        let url = "http://central.sonatype.com/api/v1/publisher/";
        assert_eq!(
            gate_error(&vars, "1.0.0", url),
            Some(format!("maven_deploy: invalid repository_url '{url}': want an https URL so uploads never go over plaintext"))
        );
    }

    fn staged_house(scratch: &Path) -> PathBuf {
        let jar = scratch.join("maven_demo-1.0.0.jar");
        std::fs::write(&jar, b"maven-demo-jar-bytes").expect("write jar");
        let pom = scratch.join("maven_demo-1.0.0.pom");
        std::fs::write(&pom, central_pom("com.example", "maven_demo", "1.0.0")).expect("pom");
        let outdir = scratch.join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        maven_build_file_repo(
            &jar,
            &pom,
            &outdir,
            "com.example",
            "maven_demo",
            "1.0.0",
            None,
        )
        .expect("repo")
    }

    #[test]
    fn maven_bundle_zip_is_deterministic_and_skips_metadata() {
        let scratch = scratch_dir();
        let house = staged_house(scratch.path());
        let first = maven_bundle_zip(&house).expect("zip");
        let second = maven_bundle_zip(&house).expect("zip again");
        assert_eq!(first, second);
        let mut archive = zip::ZipArchive::new(io::Cursor::new(first)).expect("open");
        let names: Vec<String> = (0..archive.len())
            .map(|index| {
                archive
                    .by_index(index)
                    .expect("entry")
                    .name()
                    .expect("name")
                    .into_owned()
            })
            .collect();
        let dir = "com/example/maven_demo/1.0.0/";
        let mut want = Vec::new();
        for base in ["maven_demo-1.0.0.jar", "maven_demo-1.0.0.pom"] {
            want.push(format!("{dir}{base}"));
            for suffix in ["md5", "sha1", "sha256", "sha512"] {
                want.push(format!("{dir}{base}.{suffix}"));
            }
        }
        want.sort();
        assert_eq!(names, want);
    }

    struct Stub {
        url: String,
        request: std::sync::mpsc::Receiver<Vec<u8>>,
    }

    fn stub_server(status: &str, body: String) -> Stub {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!(
            "http://{}/api/v1/publisher/",
            listener.local_addr().expect("addr")
        );
        let (send, request) = std::sync::mpsc::channel();
        let status = status.to_owned();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut seen = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = stream.read(&mut buf).expect("read");
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&seen).into_owned();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().expect("length"))
                        })
                        .unwrap_or(0);
                    if seen.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).expect("reply");
            send.send(seen).expect("send");
        });
        Stub { url, request }
    }

    fn test_agent() -> ureq::Agent {
        http::publish_agent_with_proxy(None)
    }

    #[test]
    fn maven_upload_posts_bundle_multipart() {
        let stub = stub_server("201 Created", "deployment-1234\n".to_owned());
        let zip = b"PK\x03\x04 bundle bytes".to_vec();
        let id = maven_upload_bundle(
            &test_agent(),
            &stub.url,
            &http::bearer_basic("user", "pass"),
            "com.example:demo:1.0.0",
            &zip,
        )
        .expect("upload");
        assert_eq!(id, "deployment-1234");
        let seen = stub.request.recv().expect("request");
        let text = String::from_utf8_lossy(&seen).into_owned();
        assert!(
            text.starts_with("POST /api/v1/publisher/upload?publishingType=USER_MANAGED&name=com.example%3Ademo%3A1.0.0 HTTP/1.1\r\n"),
            "{text}"
        );
        assert!(text
            .lines()
            .any(|line| line.eq_ignore_ascii_case("authorization: Bearer dXNlcjpwYXNz")));
        let boundary = text
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-type: multipart/form-data; boundary=")
                    .map(str::to_owned)
            })
            .expect("multipart content type");
        assert!(text.contains(&format!("--{boundary}\r\nContent-Disposition: form-data; name=\"bundle\"; filename=\"bundle.zip\"\r\nContent-Type: application/octet-stream\r\n\r\n")));
        assert!(seen.windows(zip.len()).any(|window| window == &zip[..]));
        assert!(text.ends_with(&format!("\r\n--{boundary}--\r\n")));
    }

    #[test]
    fn maven_upload_maps_401_and_403_to_bad_credentials() {
        for status in ["401 Unauthorized", "403 Forbidden"] {
            let stub = stub_server(status, "denied".to_owned());
            let bearer = http::bearer_basic("user", "s3cret-pass");
            let error = maven_upload_bundle(&test_agent(), &stub.url, &bearer, "g:a:1", b"zip")
                .expect_err("refused");
            assert_eq!(error, MAVEN_BAD_CREDENTIALS);
            assert!(!error.contains("s3cret-pass") && !error.contains(&bearer[7..]));
        }
    }

    #[test]
    fn maven_upload_reports_unexpected_status_with_body_excerpt() {
        let body = "x".repeat(5000);
        let stub = stub_server("400 Bad Request", body);
        let error = maven_upload_bundle(&test_agent(), &stub.url, "Bearer t", "g:a:1", b"zip")
            .expect_err("rejected");
        let prefix = format!(
            "maven_deploy: upload to {}upload?publishingType=USER_MANAGED&name=g%3Aa%3A1 failed with HTTP 400: ",
            stub.url
        );
        assert_eq!(error, prefix + &"x".repeat(2048));
    }

    #[test]
    fn maven_upload_reports_unreachable_host() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);
        let url = format!("http://{addr}/api/v1/publisher/");
        let error = maven_upload_bundle(&test_agent(), &url, "Bearer t", "g:a:1", b"zip")
            .expect_err("unreachable");
        assert!(
            error.starts_with(&format!("maven_deploy: upload to {url}upload?")),
            "{error}"
        );
        assert!(!error.contains("Bearer"));
    }

    #[test]
    fn maven_local_staging_needs_no_credentials() {
        if env_test_in_child("maven::tests::maven_local_staging_needs_no_credentials") {
            return;
        }
        let outdir = scratch_dir();
        let argv = vec![
            "prog".to_owned(),
            outdir.path().to_string_lossy().into_owned(),
        ];
        let jar = std::env::var("MAVEN_DEMO_JAR").expect("jar rloc");
        let pom = std::env::var("MAVEN_DEMO_POM").expect("pom rloc");
        let cleared = [
            "MAVEN_PUBLISH_LIVE",
            "MAVEN_PUBLISH_APPROVED",
            "MAVEN_USERNAME",
            "MAVEN_PASSWORD",
            "MAVEN_GPG_KEY",
            "MAVEN_GPG_PASSPHRASE",
            "MAVEN_ALLOW_UNSIGNED",
        ];
        let mut vars: Vec<(&str, Option<&str>)> = cleared.iter().map(|key| (*key, None)).collect();
        vars.push(("DX_PROFILE", Some("dx_dev")));
        let run = || {
            maven_main(
                &jar,
                &pom,
                "com.example",
                "maven_demo",
                "0.0.0",
                MAVEN_DEFAULT_REPOSITORY_URL,
                &argv,
            )
        };
        assert_eq!(with_env(&vars, run), 0);
        let dir = outdir
            .path()
            .join("maven_demo-repo/com/example/maven_demo/0.0.0");
        assert!(dir.join("maven_demo-0.0.0.jar.sha1").is_file());
        assert!(dir.join("maven_demo-0.0.0.pom.md5").is_file());
        assert!(!dir.join("maven_demo-0.0.0.jar.asc").exists());
        let (secret, public) = test_key(false);
        vars.push(("MAVEN_GPG_KEY", Some(&secret)));
        vars.push(("MAVEN_GPG_PASSPHRASE", Some(PASSPHRASE)));
        assert_eq!(with_env(&vars, run), 0);
        let jar_bytes = std::fs::read(dir.join("maven_demo-0.0.0.jar")).expect("jar");
        let asc = std::fs::read_to_string(dir.join("maven_demo-0.0.0.jar.asc")).expect("asc");
        maven_verify_detached(&public, &jar_bytes, &asc).expect("verifies");
        let staged: Vec<String> = std::fs::read_dir(&dir)
            .expect("list")
            .map(|entry| std::fs::read_to_string(entry.expect("entry").path()).unwrap_or_default())
            .collect();
        assert!(staged
            .iter()
            .all(|body| !body.contains("PRIVATE KEY") && !body.contains(PASSPHRASE)));
        vars.pop();
        vars.push(("MAVEN_GPG_PASSPHRASE", Some("wrong")));
        assert_eq!(with_env(&vars, run), 1);
    }

    #[test]
    fn maven_group_path_maps_dots() {
        assert_eq!(maven_group_path("com.example.demo"), "com/example/demo");
    }

    #[test]
    fn maven_repo_builds_layout() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("maven_demo-0.0.0.jar");
        std::fs::write(&jar, b"maven-demo-jar-bytes-v1").expect("write jar");
        let pom = scratch.path().join("maven_demo-0.0.0.pom");
        std::fs::write(&pom, central_pom("com.example", "maven_demo", "0.0.0")).expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house = maven_build_file_repo(
            &jar,
            &pom,
            &outdir,
            "com.example",
            "maven_demo",
            "0.0.0",
            None,
        )
        .expect("repo");
        assert!(house
            .join("com/example/maven_demo/0.0.0/maven_demo-0.0.0.jar")
            .is_file());
        assert!(house
            .join("com/example/maven_demo/maven-metadata.xml")
            .is_file());
    }

    #[test]
    fn maven_retired_ossrh_hosts_are_rejected() {
        assert_eq!(
            maven_retired_host("https://oss.sonatype.org/service/local/staging/deploy/maven2/"),
            Some("oss.sonatype.org")
        );
        assert_eq!(
            maven_retired_host("https://s01.oss.sonatype.org/service/local/"),
            Some("s01.oss.sonatype.org")
        );
        assert_eq!(maven_retired_host(MAVEN_DEFAULT_REPOSITORY_URL), None);
        assert_eq!(maven_retired_host("https://repo.example.com/maven2/"), None);
    }

    #[test]
    fn maven_file_repo_rejects_empty_coordinates() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("demo.jar");
        std::fs::write(&jar, b"jar").expect("write jar");
        let pom = scratch.path().join("demo.pom");
        std::fs::write(&pom, b"pom").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        for (group, artifact, version) in [("", "a", "1"), ("g", "", "1"), ("g", "a", "")] {
            let error = maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version, None)
                .expect_err("empty coordinate");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn maven_file_repo_rejects_escaping_coordinates() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("demo.jar");
        std::fs::write(&jar, b"jar").expect("write jar");
        let pom = scratch.path().join("demo.pom");
        std::fs::write(&pom, b"pom").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let before = std::fs::read_dir(scratch.path()).expect("list").count();
        for (group, artifact, version) in [
            (".example", "a", "1"),
            ("com.", "a", "1"),
            ("com..x", "a", "1"),
            ("com/x", "a", "1"),
            ("com\\x", "a", "1"),
            ("g", ".", "1"),
            ("g", "..", "1"),
            ("g", "a/b", "1"),
            ("g", "a", "."),
            ("g", "a", ".."),
            ("g", "a", "../x"),
        ] {
            let error = maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version, None)
                .expect_err("escaping coordinate");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
        assert_eq!(
            std::fs::read_dir(scratch.path()).expect("list").count(),
            before
        );
        assert_eq!(std::fs::read_dir(&outdir).expect("list").count(), 0);
    }

    #[test]
    fn maven_file_repo_keeps_qualified_versions() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("demo.jar");
        std::fs::write(&jar, b"jar").expect("write jar");
        let pom = scratch.path().join("demo.pom");
        std::fs::write(&pom, central_pom("com.example", "demo", "1.0-SNAPSHOT+b5"))
            .expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house = maven_build_file_repo(
            &jar,
            &pom,
            &outdir,
            "com.example",
            "demo",
            "1.0-SNAPSHOT+b5",
            None,
        )
        .expect("repo");
        let dir = house.join("com/example/demo/1.0-SNAPSHOT+b5");
        assert!(dir.join("demo-1.0-SNAPSHOT+b5.jar").is_file());
        assert!(dir.join("demo-1.0-SNAPSHOT+b5.pom.sha256").is_file());
        assert!(house.join("com/example/demo/maven-metadata.xml").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn maven_file_repo_rejects_symlinked_parents() {
        let scratch = scratch_dir();
        let jar = scratch.path().join("demo.jar");
        std::fs::write(&jar, b"jar").expect("write jar");
        let pom = scratch.path().join("demo.pom");
        std::fs::write(&pom, b"pom").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let outside = scratch.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside");
        std::fs::write(outside.join("sentinel"), b"keep").expect("sentinel");
        std::os::unix::fs::symlink(&outside, outdir.join("demo-repo")).expect("link");
        let error = maven_build_file_repo(&jar, &pom, &outdir, "com.example", "demo", "1", None)
            .expect_err("symlinked repo");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_dir(&outside).expect("list").count(), 1);
        assert_eq!(
            std::fs::read(outside.join("sentinel")).expect("read"),
            b"keep"
        );
    }
}
