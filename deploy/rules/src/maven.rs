use std::io;
use std::path::{Path, PathBuf};

use pgp::composed::{
    ArmorOptions, Deserializable, DetachedSignature, SignedPublicKey, SignedSecretKey,
};
use pgp::crypto::hash::HashAlgorithm;
use pgp::types::Password;

use crate::run::{env_or_empty, scrubbed_command, OwnerGate, PUBLISH_CHILD_ENV_KEEP};
use crate::{copy_verified, deploy_profile, tempfile_stage_dir, write_secure_file, DeployRun};

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

/// Renders one Maven settings.xml.
pub fn maven_settings_xml(username: &str, password: &str) -> String {
    let mut body = format!(
        "<settings>\n  <servers>\n    <server>\n      <id>dx-staging</id>\n      <username>{}</username>\n      <password>{}</password>\n    </server>\n  </servers>\n",
        quick_xml::escape::escape(username),
        quick_xml::escape::escape(password),
    );
    body.push_str("</settings>\n");
    body
}

/// Central Portal OSSRH staging API deploy URL.
pub const MAVEN_PORTAL_STAGING_URL: &str =
    "https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/";

/// Returns the retired OSSRH host named by a repository URL.
pub fn maven_retired_host(repository_url: &str) -> Option<&'static str> {
    let rest = repository_url.strip_prefix("https://")?;
    let host = rest.split('/').next().unwrap_or("");
    ["oss.sonatype.org", "s01.oss.sonatype.org"]
        .into_iter()
        .find(|retired| host.eq_ignore_ascii_case(retired))
}

/// Pinned maven-gpg-plugin goal that signs and uploads in one run.
pub const MAVEN_SIGN_GOAL: &str =
    "org.apache.maven.plugins:maven-gpg-plugin:3.2.8:sign-and-deploy-file";

/// Pinned maven-deploy-plugin goal for the explicit unsigned path.
pub const MAVEN_DEPLOY_GOAL: &str =
    "org.apache.maven.plugins:maven-deploy-plugin:3.1.4:deploy-file";

/// Builds the mvn sign-and-deploy-file (signed) or deploy-file (unsigned) arguments.
#[allow(clippy::too_many_arguments)]
pub fn maven_deploy_args(
    signed: bool,
    settings: &Path,
    repository_url: &str,
    jar: &Path,
    pom: &Path,
    group: &str,
    artifact: &str,
    version: &str,
) -> Vec<String> {
    let mut args = vec![
        "-B".to_owned(),
        "--settings".to_owned(),
        settings.display().to_string(),
        if signed {
            MAVEN_SIGN_GOAL
        } else {
            MAVEN_DEPLOY_GOAL
        }
        .to_owned(),
        format!("-Durl={repository_url}"),
        "-DrepositoryId=dx-staging".to_owned(),
        format!("-Dfile={}", jar.display()),
        format!("-DpomFile={}", pom.display()),
        format!("-DgroupId={group}"),
        format!("-DartifactId={artifact}"),
        format!("-Dversion={version}"),
        "-Dpackaging=jar".to_owned(),
    ];
    if signed {
        args.extend([
            "-Dgpg.signer=bc".to_owned(),
            "-Dgpg.bestPractices=true".to_owned(),
            "-Dgpg.useagent=false".to_owned(),
        ]);
    }
    args
}

/// Owner approval gate for maven_deploy.
pub(crate) const MAVEN_GATE: OwnerGate = OwnerGate {
    tool: "maven_deploy",
    action: "staging",
    credentials: &["MAVEN_USERNAME", "MAVEN_PASSWORD"],
    approved: "MAVEN_PUBLISH_APPROVED",
    live: "MAVEN_PUBLISH_LIVE",
};

/// Extra env names `mvn` keeps beyond the publish base list.
const MAVEN_CHILD_ENV_EXTRA: &[&str] = &[
    "JAVA_HOME",
    "MAVEN_HOME",
    "M2_HOME",
    "MAVEN_OPTS",
    "MAVEN_ARGS",
    "MAVEN_GPG_KEY_FINGERPRINT",
];

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
    /// Armored key from MAVEN_GPG_KEY, empty when unsigned.
    pub gpg_key: String,
    /// Passphrase from MAVEN_GPG_PASSPHRASE, empty when unsigned.
    pub gpg_passphrase: String,
}

/// Builds the mvn command in a cleared env.
///
/// A signed upload gets MAVEN_GPG_KEY and MAVEN_GPG_PASSPHRASE back, because the
/// bc signer reads them from env; the repository password stays in settings.xml.
pub fn maven_command(mvn: &Path, live: &MavenLive, args: &[String]) -> std::process::Command {
    let mut extra: Vec<(&str, &str)> = Vec::new();
    if !live.unsigned {
        extra.push(("MAVEN_GPG_KEY", &live.gpg_key));
        if !live.gpg_passphrase.is_empty() {
            extra.push(("MAVEN_GPG_PASSPHRASE", &live.gpg_passphrase));
        }
    }
    let mut cmd = scrubbed_command(
        mvn,
        &[PUBLISH_CHILD_ENV_KEEP, MAVEN_CHILD_ENV_EXTRA],
        &extra,
    );
    cmd.args(args);
    cmd
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
    let secrets = MAVEN_GATE.authorize(env)?;
    if let Some(host) = maven_retired_host(repository_url) {
        return Err(format!(
            "maven_deploy: invalid repository_url '{repository_url}': {host} is retired OSSRH infrastructure (use '{MAVEN_PORTAL_STAGING_URL}')"
        ));
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
    let (gpg_key, gpg_passphrase) = if unsigned {
        (String::new(), String::new())
    } else {
        (key, passphrase)
    };
    Ok(MavenLive {
        username: secrets.get("MAVEN_USERNAME").to_owned(),
        password: secrets.get("MAVEN_PASSWORD").to_owned(),
        signer,
        unsigned,
        gpg_key,
        gpg_passphrase,
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
        match std::fs::read(&pom) {
            Ok(bytes) => {
                if let Some(error) = maven_pom_metadata_error(&bytes, group, artifact, version) {
                    eprintln!("maven_deploy: {error}");
                    return 1;
                }
            }
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        }
        let workdir = match tempfile_stage_dir("maven-deploy-") {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                return 1;
            }
        };
        let settings = match write_secure_file(
            &workdir,
            "settings.xml",
            &maven_settings_xml(&live.username, &live.password),
        ) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                let _ = std::fs::remove_dir_all(&workdir);
                return 1;
            }
        };
        let mut cmd = maven_command(
            Path::new("mvn"),
            &live,
            &maven_deploy_args(
                !live.unsigned,
                &settings,
                repository_url,
                &jar,
                &pom,
                group,
                artifact,
                version,
            ),
        );
        if live.unsigned {
            println!(
                "maven_deploy: unsigned staging recorded (MAVEN_ALLOW_UNSIGNED=1 with owner approval; no GPG signature attached)"
            );
        }
        let status = cmd.status();
        let _ = std::fs::remove_dir_all(&workdir);
        match status {
            Ok(status) if status.success() => {
                println!("maven_deploy: uploaded {group}:{artifact}:{version} to {repository_url}");
                println!(
                    "maven_deploy: not published yet; validate and publish the deployment in the Central Portal"
                );
                return 0;
            }
            Ok(status) => {
                eprintln!("maven_deploy: mvn failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("maven_deploy: {error}");
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
    use crate::tests::{env_test_in_child, scratch_dir, with_env};
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
            gate_error(&vars, "0.0.0", MAVEN_PORTAL_STAGING_URL),
            Some("maven_deploy: live staging refuses version 0.0.0; set a real version".to_owned())
        );
    }

    #[test]
    fn maven_gate_rejects_missing_credentials() {
        for vars in [
            &[("MAVEN_PASSWORD", "pass"), ("MAVEN_PUBLISH_APPROVED", "1")][..],
            &[("MAVEN_USERNAME", "user"), ("MAVEN_PUBLISH_APPROVED", "1")][..],
        ] {
            assert_eq!(
                gate_error(vars, "1.0.0", MAVEN_PORTAL_STAGING_URL),
                Some("maven_deploy: live staging needs MAVEN_USERNAME plus MAVEN_PASSWORD plus explicit owner approval (MAVEN_PUBLISH_APPROVED=1); refusing".to_owned())
            );
        }
    }

    #[test]
    fn maven_gate_rejects_missing_approval() {
        assert_eq!(
            gate_error(&APPROVED[..2], "1.0.0", MAVEN_PORTAL_STAGING_URL),
            Some(
                "maven_deploy: live staging needs MAVEN_PUBLISH_APPROVED=1 (owner approval); refusing"
                    .to_owned()
            )
        );
    }

    #[test]
    fn maven_gate_rejects_unsigned_without_allow_and_accepts_with_allow() {
        assert_eq!(
            gate_error(&APPROVED, "1.0.0", MAVEN_PORTAL_STAGING_URL),
            Some("maven staging: no MAVEN_GPG_KEY; refusing unsigned publish (set MAVEN_GPG_KEY and MAVEN_GPG_PASSPHRASE, or record unsigned staging explicitly with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)".to_owned())
        );
        let mut vars = APPROVED.to_vec();
        vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        let live = maven_live_gate(&gate_env(&vars), "1.0.0", MAVEN_PORTAL_STAGING_URL)
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
        let live = maven_live_gate(&gate_env(&vars), "1.0.0", MAVEN_PORTAL_STAGING_URL)
            .ok()
            .expect("signed");
        assert!(!live.unsigned);
        assert!(live.signer.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn maven_command_scrubs_env_and_passes_gpg_only_when_signed() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = scratch_dir();
        let mvn = scratch.path().join("mvn");
        let d = scratch.path().display();
        std::fs::write(
            &mvn,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\n"),
        )
        .expect("write mvn");
        std::fs::set_permissions(&mvn, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let (secret, _) = test_key(false);
        let mut signed = APPROVED.to_vec();
        signed.push(("MAVEN_GPG_KEY", &secret));
        signed.push(("MAVEN_GPG_PASSPHRASE", PASSPHRASE));
        let mut unsigned = APPROVED.to_vec();
        unsigned.push(("MAVEN_ALLOW_UNSIGNED", "1"));
        let settings = scratch.path().join("settings.xml");
        for (vars, want_gpg) in [(signed, true), (unsigned, false)] {
            let live = maven_live_gate(&gate_env(&vars), "1.0.0", MAVEN_PORTAL_STAGING_URL)
                .ok()
                .expect("gate");
            let args = maven_deploy_args(
                !live.unsigned,
                &settings,
                MAVEN_PORTAL_STAGING_URL,
                Path::new("a.jar"),
                Path::new("pom.xml"),
                "com.example",
                "demo",
                "1.0.0",
            );
            let status = with_env(
                &[
                    ("DX_LEAK_PROBE", Some("1")),
                    ("MAVEN_PASSWORD", Some("pass")),
                    ("MAVEN_GPG_KEY", None),
                    ("MAVEN_GPG_PASSPHRASE", None),
                    ("JAVA_HOME", Some("/opt/jdk")),
                ],
                || maven_command(&mvn, &live, &args).status(),
            )
            .expect("run mvn");
            assert!(status.success());
            let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
            let argv = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
            assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
            assert!(!env.contains("MAVEN_PASSWORD"), "{env}");
            assert!(env.lines().any(|line| line == "JAVA_HOME=/opt/jdk"));
            assert_eq!(env.contains("MAVEN_GPG_KEY="), want_gpg, "{env}");
            assert_eq!(
                env.lines()
                    .any(|line| line == format!("MAVEN_GPG_PASSPHRASE={PASSPHRASE}")),
                want_gpg
            );
            assert!(
                !argv
                    .lines()
                    .any(|line| line == "pass" || line.ends_with("=pass")),
                "{argv}"
            );
            assert!(
                !argv.contains(PASSPHRASE) && !argv.contains("PGP"),
                "{argv}"
            );
        }
    }

    #[test]
    fn maven_gate_rejects_passphrase_without_key() {
        for allow in [false, true] {
            let mut vars = APPROVED.to_vec();
            vars.push(("MAVEN_GPG_PASSPHRASE", PASSPHRASE));
            if allow {
                vars.push(("MAVEN_ALLOW_UNSIGNED", "1"));
            }
            let error = gate_error(&vars, "1.0.0", MAVEN_PORTAL_STAGING_URL).expect("refused");
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
            let error = gate_error(&vars, "1.0.0", MAVEN_PORTAL_STAGING_URL).expect("refused");
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
                Some(format!("maven_deploy: invalid repository_url '{url}': {host} is retired OSSRH infrastructure (use '{MAVEN_PORTAL_STAGING_URL}')"))
            );
        }
        assert_eq!(gate_error(&vars, "1.0.0", MAVEN_PORTAL_STAGING_URL), None);
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
                MAVEN_PORTAL_STAGING_URL,
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
    fn maven_repo_builds_layout_and_settings() {
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
        let settings = maven_settings_xml("user", "pass");
        assert!(settings.contains("dx-staging"));
        assert!(!settings.contains("profile"));
        assert!(!settings.contains("gpg.passphrase"));
        let secure = write_secure_file(scratch.path(), "settings.xml", &settings).expect("secure");
        assert!(secure.is_file());
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
        assert_eq!(maven_retired_host(MAVEN_PORTAL_STAGING_URL), None);
        assert_eq!(maven_retired_host("https://repo.example.com/maven2/"), None);
    }

    fn deploy_args(signed: bool) -> Vec<String> {
        maven_deploy_args(
            signed,
            Path::new("/tmp/settings.xml"),
            MAVEN_PORTAL_STAGING_URL,
            Path::new("/tmp/a.jar"),
            Path::new("/tmp/a.pom"),
            "com.example",
            "a",
            "1.0.0",
        )
    }

    fn assert_common_deploy_args(args: &[String]) {
        assert!(args.contains(&format!("-Durl={MAVEN_PORTAL_STAGING_URL}")));
        assert!(args.contains(&"-Dfile=/tmp/a.jar".to_owned()));
        assert!(args.contains(&"-Dversion=1.0.0".to_owned()));
        assert!(!args
            .iter()
            .any(|arg| arg.contains("-Pgpg-sign") || arg.contains("gpg.skip")));
        assert!(!args.iter().any(|arg| {
            let lower = arg.to_lowercase();
            lower.contains("password") || lower.contains("passphrase") || lower.contains("key")
        }));
    }

    #[test]
    fn maven_deploy_args_sign_when_signed() {
        let args = deploy_args(true);
        assert_common_deploy_args(&args);
        assert!(args.contains(&MAVEN_SIGN_GOAL.to_owned()));
        assert!(!args.contains(&MAVEN_DEPLOY_GOAL.to_owned()));
        for flag in [
            "-Dgpg.signer=bc",
            "-Dgpg.bestPractices=true",
            "-Dgpg.useagent=false",
        ] {
            assert!(args.contains(&flag.to_owned()), "{flag}");
        }
    }

    #[test]
    fn maven_deploy_args_unsigned_use_deploy_file() {
        let args = deploy_args(false);
        assert_common_deploy_args(&args);
        assert!(args.contains(&MAVEN_DEPLOY_GOAL.to_owned()));
        assert!(!args.contains(&MAVEN_SIGN_GOAL.to_owned()));
        assert!(!args.iter().any(|arg| arg.contains("gpg.")));
        let strip = |args: Vec<String>| -> Vec<String> {
            args.into_iter()
                .filter(|arg| {
                    !arg.starts_with("-Dgpg.") && arg != MAVEN_SIGN_GOAL && arg != MAVEN_DEPLOY_GOAL
                })
                .collect()
        };
        assert_eq!(strip(deploy_args(true)), strip(args));
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
