use std::io;
use std::path::Path;

use super::{
    copy_verified, deploy_outdir, env_or_empty, resolve_runfile, tempfile_stage_dir,
    write_secure_file, OwnerGate,
};

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
    let jar = format!("{artifact}-{version}.jar");
    let pom = format!("{artifact}-{version}.pom");
    for base in [jar.clone(), jar + ".sha256", pom.clone(), pom + ".sha256"] {
        check(&current.join(base))?;
    }
    check(
        &current
            .parent()
            .unwrap_or(&current)
            .join("maven-metadata.xml"),
    )
}

/// Builds one Maven file repo.
pub fn maven_build_file_repo(
    jar_src: &Path,
    pom_src: &Path,
    outdir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
) -> io::Result<std::path::PathBuf> {
    maven_coordinates_check(group, artifact, version)?;
    let rel = std::path::PathBuf::from(artifact.to_owned() + "-repo")
        .join(maven_group_path(group))
        .join(artifact)
        .join(version);
    maven_reject_symlinks(outdir, &rel, artifact, version)?;
    let house = outdir.join(artifact.to_owned() + "-repo");
    let dest_dir = house
        .join(maven_group_path(group))
        .join(artifact)
        .join(version);
    std::fs::create_dir_all(&dest_dir)?;
    let jar_base = format!("{artifact}-{version}.jar");
    let pom_base = format!("{artifact}-{version}.pom");
    for (src, base) in [(jar_src, jar_base.clone()), (pom_src, pom_base.clone())] {
        let dest = dest_dir.join(&base);
        let digest = copy_verified(src, &dest)?;
        std::fs::write(
            dest_dir.join(base.clone() + ".sha256"),
            format!("{digest}  {base}\n").as_bytes(),
        )?;
    }
    let metadata_dir = house.join(maven_group_path(group)).join(artifact);
    std::fs::create_dir_all(&metadata_dir)?;
    let metadata = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<metadata>\n  <groupId>{group}</groupId>\n  <artifactId>{artifact}</artifactId>\n  <version>{version}</version>\n  <versioning>\n    <latest>{version}</latest>\n    <release>{version}</release>\n    <versions>\n      <version>{version}</version>\n    </versions>\n  </versioning>\n</metadata>\n"
    );
    std::fs::write(metadata_dir.join("maven-metadata.xml"), metadata.as_bytes())?;
    Ok(house)
}

/// Renders one Maven settings.xml.
pub fn maven_settings_xml(username: &str, password: &str, passphrase: &str) -> String {
    let mut body = format!(
        "<settings>\n  <servers>\n    <server>\n      <id>dx-staging</id>\n      <username>{}</username>\n      <password>{}</password>\n    </server>\n  </servers>\n",
        quick_xml::escape::escape(username),
        quick_xml::escape::escape(password),
    );
    if !passphrase.is_empty() {
        body.push_str(&format!(
            "  <profiles>\n    <profile>\n      <id>dx-gpg-passphrase</id>\n      <properties>\n        <gpg.passphrase>{}</gpg.passphrase>\n      </properties>\n    </profile>\n  </profiles>\n  <activeProfiles>\n    <activeProfile>dx-gpg-passphrase</activeProfile>\n  </activeProfiles>\n",
            quick_xml::escape::escape(passphrase),
        ));
    }
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

/// Builds the mvn deploy-file arguments.
pub fn maven_deploy_args(
    settings: &Path,
    repository_url: &str,
    jar: &Path,
    pom: &Path,
    group: &str,
    artifact: &str,
    version: &str,
) -> Vec<String> {
    vec![
        "-B".to_owned(),
        "--settings".to_owned(),
        settings.display().to_string(),
        "deploy:deploy-file".to_owned(),
        format!("-Durl={repository_url}"),
        "-DrepositoryId=dx-staging".to_owned(),
        format!("-Dfile={}", jar.display()),
        format!("-DpomFile={}", pom.display()),
        format!("-DgroupId={group}"),
        format!("-DartifactId={artifact}"),
        format!("-Dversion={version}"),
        "-Dpackaging=jar".to_owned(),
    ]
}

/// Owner approval gate for maven_deploy.
const MAVEN_GATE: OwnerGate = OwnerGate {
    tool: "maven_deploy",
    action: "staging",
    credentials: &["MAVEN_USERNAME", "MAVEN_PASSWORD"],
    approved: "MAVEN_PUBLISH_APPROVED",
};

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
    let outdir = deploy_outdir(argv);
    if std::fs::create_dir_all(&outdir).is_err() {
        eprintln!("maven_deploy: cannot create {}", outdir.display());
        return 1;
    }
    let jar = match resolve_runfile(jar_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            return 1;
        }
    };
    let pom = match resolve_runfile(pom_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            return 1;
        }
    };
    if std::env::var("MAVEN_PUBLISH_LIVE").unwrap_or_default() == "1" {
        let username = std::env::var("MAVEN_USERNAME").unwrap_or_default();
        let password = std::env::var("MAVEN_PASSWORD").unwrap_or_default();
        if version == "0.0.0" {
            eprintln!("maven_deploy: live staging refuses version 0.0.0; set a real version");
            return 1;
        }
        if let Some(error) = MAVEN_GATE.refusal(env_or_empty) {
            eprintln!("{error}");
            return 1;
        }
        let passphrase = std::env::var("MAVEN_GPG_PASSPHRASE").unwrap_or_default();
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
            &maven_settings_xml(&username, &password, &passphrase),
        ) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("maven_deploy: {error}");
                let _ = std::fs::remove_dir_all(&workdir);
                return 1;
            }
        };
        let mut cmd = std::process::Command::new("mvn");
        cmd.args(maven_deploy_args(
            &settings,
            repository_url,
            &jar,
            &pom,
            group,
            artifact,
            version,
        ));
        if !passphrase.is_empty() {
            cmd.arg("-Pgpg-sign");
        } else {
            if std::env::var("MAVEN_ALLOW_UNSIGNED").unwrap_or_default() != "1" {
                eprintln!(
                    "maven staging: no MAVEN_GPG_PASSPHRASE; refusing unsigned publish (set MAVEN_GPG_PASSPHRASE, or record unsigned staging explicitly with MAVEN_ALLOW_UNSIGNED=1 plus owner approval)"
                );
                let _ = std::fs::remove_dir_all(&workdir);
                return 1;
            }
            cmd.arg("-Dgpg.skip=true");
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
    match maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version) {
        Ok(house) => {
            let profile = std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned());
            println!("maven_deploy: staged {group}:{artifact}:{version} (profile: {profile})");
            println!("  repo: {}", house.display());
            println!("  url: file://{}", house.display());
            0
        }
        Err(error) => {
            eprintln!("maven_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::scratch_dir;

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
        std::fs::write(&pom, b"<project>maven-demo-pom-v1</project>").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house =
            maven_build_file_repo(&jar, &pom, &outdir, "com.example", "maven_demo", "0.0.0")
                .expect("repo");
        assert!(house
            .join("com/example/maven_demo/0.0.0/maven_demo-0.0.0.jar")
            .is_file());
        assert!(house
            .join("com/example/maven_demo/maven-metadata.xml")
            .is_file());
        let settings = maven_settings_xml("user", "pass", "secret");
        assert!(settings.contains("dx-staging"));
        assert!(settings.contains("gpg.passphrase"));
        let plain = maven_settings_xml("user", "pass", "");
        assert!(!plain.contains("gpg.passphrase"));
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

    #[test]
    fn maven_deploy_args_target_the_given_repository() {
        let args = maven_deploy_args(
            Path::new("/tmp/settings.xml"),
            MAVEN_PORTAL_STAGING_URL,
            Path::new("/tmp/a.jar"),
            Path::new("/tmp/a.pom"),
            "com.example",
            "a",
            "1.0.0",
        );
        assert!(args.contains(&format!("-Durl={MAVEN_PORTAL_STAGING_URL}")));
        assert!(args.contains(&"deploy:deploy-file".to_owned()));
        assert!(args.contains(&"-Dfile=/tmp/a.jar".to_owned()));
        assert!(args.contains(&"-Dversion=1.0.0".to_owned()));
        assert!(!args.iter().any(|arg| arg.contains("password")));
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
            let error = maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version)
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
            let error = maven_build_file_repo(&jar, &pom, &outdir, group, artifact, version)
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
        std::fs::write(&pom, b"pom").expect("write pom");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let house = maven_build_file_repo(
            &jar,
            &pom,
            &outdir,
            "com.example",
            "demo",
            "1.0-SNAPSHOT+b5",
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
        let error = maven_build_file_repo(&jar, &pom, &outdir, "com.example", "demo", "1")
            .expect_err("symlinked repo");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_dir(&outside).expect("list").count(), 1);
        assert_eq!(
            std::fs::read(outside.join("sentinel")).expect("read"),
            b"keep"
        );
    }
}
