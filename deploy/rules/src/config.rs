//! Reads the JSON deploy config that the shared deploy_tool runs from.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use serde::de::DeserializeOwned;

/// Names the variable that holds the deploy config rlocation or absolute path.
pub const DEPLOY_CONFIG_ENV: &str = "DX_DEPLOY_CONFIG";

/// Pins one archive deploy: the app name file, the tarball and its checksum.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveConfig {
    pub app_rloc: String,
    pub tarball_rloc: String,
    pub checksum_rloc: String,
}

/// Pins one draft GitHub Release: its tag and asset files.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubConfig {
    pub deploy_name: String,
    pub tag: String,
    pub asset_rlocs: Vec<String>,
}

/// Pins one Cloudflare Pages deploy: the project, branches, site and Wrangler.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudflarePagesConfig {
    pub deploy_name: String,
    pub account_id: String,
    pub project: String,
    pub branch: String,
    pub production_branch: String,
    pub site_rloc: String,
    pub wrangler_rloc: String,
}

/// Pins one PyPI upload: the wheel, an optional sdist and the index.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PypiConfig {
    pub wheel_rloc: String,
    #[serde(default)]
    pub sdist_rloc: Option<String>,
    pub dist_name: String,
    pub repository_url: String,
}

/// Pins one npm publish: the packed feed and tarball.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpmConfig {
    pub feed_rloc: String,
    pub tarball_rloc: String,
}

/// Pins one NuGet push: the nupkg, its expected id and version, and the feed.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NugetConfig {
    pub nupkg_rloc: String,
    pub deploy_name: String,
    pub package_id: String,
    pub version: String,
    pub source: String,
}

/// Pins one crates.io publish: the crate files, name and version.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CratesConfig {
    pub crate_rlocs: Vec<String>,
    pub crate_name: String,
    pub version: String,
}

/// Pins one Maven publish: the jar, pom, optional extra jars and the repository.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MavenConfig {
    pub jar_rloc: String,
    pub pom_rloc: String,
    #[serde(default)]
    pub sources_rloc: Option<String>,
    #[serde(default)]
    pub javadoc_rloc: Option<String>,
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub repository_url: String,
}

/// Pins one OCI push: the image and its destination.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OciConfig {
    pub image_rloc: String,
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub platform: String,
}

/// Pins one Octopus release: the package, project and target environments.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OctopusConfig {
    pub package_rloc: String,
    pub deploy_name: String,
    pub project: String,
    pub channel: String,
    pub version: String,
    pub deploy_to: Vec<String>,
    pub space: String,
    pub octopus_url: String,
}

/// Pins one promotion: the artifact and the environment edge.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionConfig {
    pub artifact_rloc: String,
    pub deploy_name: String,
    pub from_environment: String,
    pub to_environment: String,
    pub version: String,
}

/// Holds one deploy config; `kind` picks the variant and `config` holds its fields.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(
    tag = "kind",
    content = "config",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DeployConfig {
    Archive(ArchiveConfig),
    Github(GithubConfig),
    CloudflarePages(CloudflarePagesConfig),
    Pypi(PypiConfig),
    Npm(NpmConfig),
    Nuget(NugetConfig),
    Crates(CratesConfig),
    Maven(MavenConfig),
    Oci(OciConfig),
    Octopus(OctopusConfig),
    Promotion(PromotionConfig),
}

/// Reads the deploy config named by `DX_DEPLOY_CONFIG`, then clears the variable.
///
/// Clearing it keeps child processes from inheriting it.
pub fn load_deploy_config<T: DeserializeOwned>() -> Result<T, String> {
    let raw = std::env::var_os(DEPLOY_CONFIG_ENV);
    std::env::remove_var(DEPLOY_CONFIG_ENV);
    load_deploy_config_from(raw, &crate::resolve_runfile)
}

/// Reads one deploy config from an absolute path or a runfiles rlocation.
pub fn load_deploy_config_from<T: DeserializeOwned>(
    raw: Option<OsString>,
    resolve: &dyn Fn(&str) -> io::Result<PathBuf>,
) -> Result<T, String> {
    let Some(raw) = raw.filter(|value| !value.is_empty()) else {
        return Err(format!(
            "{DEPLOY_CONFIG_ENV} not set: run with bazel run or dx deploy"
        ));
    };
    let direct = PathBuf::from(&raw);
    let path = if direct.is_absolute() {
        direct
    } else {
        let Some(rloc) = raw.to_str() else {
            return Err(format!(
                "{DEPLOY_CONFIG_ENV} is not valid UTF-8: {}",
                raw.to_string_lossy()
            ));
        };
        resolve(rloc)
            .map_err(|error| format!("cannot resolve {DEPLOY_CONFIG_ENV} '{rloc}': {error}"))?
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read deploy config {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("invalid deploy config {}: {error}", path.display()))
}

/// Runs the deploy that one config names.
pub fn run_deploy_config(config: &DeployConfig, argv: &[String]) -> i32 {
    match config {
        DeployConfig::Archive(archive) => crate::archive_main(
            &archive.app_rloc,
            &archive.tarball_rloc,
            &archive.checksum_rloc,
            argv,
        ),
        DeployConfig::Github(github) => {
            crate::github_main(&github.asset_rlocs, &github.deploy_name, &github.tag, argv)
        }
        DeployConfig::CloudflarePages(pages) => crate::cloudflare_pages_main(
            &crate::PagesLaunch {
                deploy_name: &pages.deploy_name,
                account_id: &pages.account_id,
                project: &pages.project,
                branch: &pages.branch,
                production_branch: &pages.production_branch,
                site_rloc: &pages.site_rloc,
                wrangler_rloc: &pages.wrangler_rloc,
            },
            argv,
        ),
        DeployConfig::Pypi(pypi) => crate::pypi_main(
            &pypi.wheel_rloc,
            pypi.sdist_rloc.as_deref().unwrap_or(""),
            &pypi.dist_name,
            &pypi.repository_url,
            argv,
        ),
        DeployConfig::Npm(npm) => crate::npm_deploy_main(argv, &npm.feed_rloc, &npm.tarball_rloc),
        DeployConfig::Nuget(nuget) => crate::nuget_main(
            &nuget.nupkg_rloc,
            &nuget.deploy_name,
            &nuget.package_id,
            &nuget.version,
            &nuget.source,
            argv,
        ),
        DeployConfig::Crates(crates) => crate::crates_main(
            &crates.crate_rlocs,
            &crates.crate_name,
            &crates.version,
            argv,
        ),
        DeployConfig::Maven(maven) => crate::maven_main(
            &crate::MavenLaunch {
                jar_rloc: &maven.jar_rloc,
                pom_rloc: &maven.pom_rloc,
                sources_rloc: maven.sources_rloc.as_deref().unwrap_or(""),
                javadoc_rloc: maven.javadoc_rloc.as_deref().unwrap_or(""),
                group: &maven.group,
                artifact: &maven.artifact,
                version: &maven.version,
                repository_url: &maven.repository_url,
            },
            argv,
        ),
        DeployConfig::Oci(oci) => crate::oci_main(
            &oci.image_rloc,
            &oci.registry,
            &oci.repository,
            &oci.tag,
            &oci.platform,
            argv,
        ),
        DeployConfig::Octopus(octopus) => crate::octopus_main(
            &crate::OctopusLaunch {
                package_rloc: &octopus.package_rloc,
                deploy_name: &octopus.deploy_name,
                project: &octopus.project,
                channel: &octopus.channel,
                version: &octopus.version,
                deploy_to: &octopus.deploy_to,
                space_default: &octopus.space,
                url_default: &octopus.octopus_url,
            },
            argv,
        ),
        DeployConfig::Promotion(promotion) => crate::promotion_main(
            &promotion.artifact_rloc,
            &promotion.deploy_name,
            &promotion.from_environment,
            &promotion.to_environment,
            &promotion.version,
            argv,
        ),
    }
}

/// Runs the shared deploy tool: loads the config, then dispatches on its kind.
///
/// Returns 2 when the config cannot be loaded.
pub fn deploy_tool_main(argv: &[String]) -> i32 {
    match load_deploy_config::<DeployConfig>() {
        Ok(config) => run_deploy_config(&config, argv),
        Err(error) => {
            eprintln!("deploy_tool: {error}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{scratch_dir, with_env};

    fn parse(text: &str) -> Result<DeployConfig, String> {
        let dir = scratch_dir();
        let path = dir.path().join("demo.deploy.json");
        std::fs::write(&path, text).expect("write config");
        load_deploy_config_from(Some(path.into_os_string()), &|_| {
            Err(io::Error::new(io::ErrorKind::NotFound, "unused"))
        })
    }

    fn no_runfiles(_: &str) -> io::Result<PathBuf> {
        Err(io::Error::new(io::ErrorKind::NotFound, "runfile not found"))
    }

    #[test]
    fn an_archive_config_round_trips() {
        let config = parse(
            r#"{"kind":"archive","config":{"app_rloc":"_main/a.txt","tarball_rloc":"_main/a.tar.gz","checksum_rloc":"_main/a.tar.gz.sha256"}}"#,
        );
        assert_eq!(
            config,
            Ok(DeployConfig::Archive(ArchiveConfig {
                app_rloc: "_main/a.txt".to_owned(),
                tarball_rloc: "_main/a.tar.gz".to_owned(),
                checksum_rloc: "_main/a.tar.gz.sha256".to_owned(),
            }))
        );
    }

    #[test]
    fn a_github_config_round_trips() {
        let config = parse(
            r#"{"config":{"deploy_name":"demo","tag":"v1.0.0","asset_rlocs":["_main/a","x;y"]},"kind":"github"}"#,
        );
        assert_eq!(
            config,
            Ok(DeployConfig::Github(GithubConfig {
                deploy_name: "demo".to_owned(),
                tag: "v1.0.0".to_owned(),
                asset_rlocs: vec!["_main/a".to_owned(), "x;y".to_owned()],
            }))
        );
    }

    #[test]
    fn a_cloudflare_config_round_trips() {
        let config = parse(
            r#"{"kind":"cloudflare_pages","config":{"deploy_name":"demo","account_id":"0123","project":"p","branch":"feature/\"q\"","production_branch":"main","site_rloc":"_main/site","wrangler_rloc":"_main/wrangler"}}"#,
        );
        assert_eq!(
            config,
            Ok(DeployConfig::CloudflarePages(CloudflarePagesConfig {
                deploy_name: "demo".to_owned(),
                account_id: "0123".to_owned(),
                project: "p".to_owned(),
                branch: "feature/\"q\"".to_owned(),
                production_branch: "main".to_owned(),
                site_rloc: "_main/site".to_owned(),
                wrangler_rloc: "_main/wrangler".to_owned(),
            }))
        );
    }

    #[test]
    fn the_new_kinds_round_trip() {
        assert_eq!(
            parse(
                r#"{"kind":"pypi","config":{"wheel_rloc":"w","sdist_rloc":"s","dist_name":"d","repository_url":"https://x"}}"#
            ),
            Ok(DeployConfig::Pypi(PypiConfig {
                wheel_rloc: "w".to_owned(),
                sdist_rloc: Some("s".to_owned()),
                dist_name: "d".to_owned(),
                repository_url: "https://x".to_owned(),
            }))
        );
        assert_eq!(
            parse(r#"{"kind":"npm","config":{"feed_rloc":"f","tarball_rloc":"t"}}"#),
            Ok(DeployConfig::Npm(NpmConfig {
                feed_rloc: "f".to_owned(),
                tarball_rloc: "t".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"nuget","config":{"nupkg_rloc":"n","deploy_name":"d","package_id":"","version":"1.0.0","source":"https://x"}}"#
            ),
            Ok(DeployConfig::Nuget(NugetConfig {
                nupkg_rloc: "n".to_owned(),
                deploy_name: "d".to_owned(),
                package_id: String::new(),
                version: "1.0.0".to_owned(),
                source: "https://x".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"crates","config":{"crate_rlocs":["a;b","c,d"],"crate_name":"c","version":"1.0.0"}}"#
            ),
            Ok(DeployConfig::Crates(CratesConfig {
                crate_rlocs: vec!["a;b".to_owned(), "c,d".to_owned()],
                crate_name: "c".to_owned(),
                version: "1.0.0".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"maven","config":{"jar_rloc":"j","pom_rloc":"p","sources_rloc":"s","group":"g","artifact":"a","version":"1","repository_url":"https://x"}}"#
            ),
            Ok(DeployConfig::Maven(MavenConfig {
                jar_rloc: "j".to_owned(),
                pom_rloc: "p".to_owned(),
                sources_rloc: Some("s".to_owned()),
                javadoc_rloc: None,
                group: "g".to_owned(),
                artifact: "a".to_owned(),
                version: "1".to_owned(),
                repository_url: "https://x".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"oci","config":{"image_rloc":"i","registry":"r","repository":"p","tag":"t","platform":"linux/amd64"}}"#
            ),
            Ok(DeployConfig::Oci(OciConfig {
                image_rloc: "i".to_owned(),
                registry: "r".to_owned(),
                repository: "p".to_owned(),
                tag: "t".to_owned(),
                platform: "linux/amd64".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"octopus","config":{"package_rloc":"p","deploy_name":"d","project":"x","channel":"c","version":"1","deploy_to":["A,B","C"],"space":"","octopus_url":"https://x"}}"#
            ),
            Ok(DeployConfig::Octopus(OctopusConfig {
                package_rloc: "p".to_owned(),
                deploy_name: "d".to_owned(),
                project: "x".to_owned(),
                channel: "c".to_owned(),
                version: "1".to_owned(),
                deploy_to: vec!["A,B".to_owned(), "C".to_owned()],
                space: String::new(),
                octopus_url: "https://x".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"promotion","config":{"artifact_rloc":"a","deploy_name":"d","from_environment":"s","to_environment":"p","version":"1"}}"#
            ),
            Ok(DeployConfig::Promotion(PromotionConfig {
                artifact_rloc: "a".to_owned(),
                deploy_name: "d".to_owned(),
                from_environment: "s".to_owned(),
                to_environment: "p".to_owned(),
                version: "1".to_owned(),
            }))
        );
    }

    #[test]
    fn a_missing_optional_rloc_parses_to_none() {
        let config = parse(
            r#"{"kind":"pypi","config":{"wheel_rloc":"w","dist_name":"d","repository_url":"https://x"}}"#,
        );
        assert!(
            matches!(&config, Ok(DeployConfig::Pypi(pypi)) if pypi.sdist_rloc.is_none()),
            "{config:?}"
        );
    }

    #[test]
    fn an_unknown_field_on_a_new_kind_is_rejected() {
        let error = parse(
            r#"{"kind":"oci","config":{"image_rloc":"i","registry":"r","repository":"p","tag":"t","platform":"x","extra":1}}"#,
        )
        .expect_err("unknown field");
        assert!(error.contains("unknown field `extra`"), "{error}");
    }

    #[test]
    fn bad_configs_fail_with_a_clear_message() {
        for (text, want) in [
            (r#"{"kind":"ftp","config":{}}"#, "unknown variant `ftp`"),
            (
                r#"{"kind":"archive","config":{"app_rloc":"a","tarball_rloc":"b","checksum_rloc":"c","extra":"d"}}"#,
                "unknown field `extra`",
            ),
            (
                r#"{"kind":"archive","config":{"app_rloc":"a","tarball_rloc":"b"}}"#,
                "missing field `checksum_rloc`",
            ),
            (
                r#"{"kind":"github","config":{"deploy_name":"a","tag":"b","asset_rlocs":[]},"extra":1}"#,
                "string \"extra\", expected \"kind\" or \"config\"",
            ),
            ("not json", "invalid deploy config"),
        ] {
            let error = parse(text).expect_err(text);
            assert!(error.contains("invalid deploy config"), "{error}");
            assert!(error.contains(want), "{text}: {error}");
        }
    }

    #[test]
    fn a_missing_variable_names_how_to_run() {
        for raw in [None, Some(OsString::new())] {
            let error = load_deploy_config_from::<DeployConfig>(raw, &no_runfiles)
                .expect_err("unset config");
            assert_eq!(
                error,
                "DX_DEPLOY_CONFIG not set: run with bazel run or dx deploy"
            );
        }
    }

    #[test]
    fn a_bad_path_names_the_path() {
        let dir = scratch_dir();
        let absent = dir.path().join("absent.deploy.json");
        let error = load_deploy_config_from::<DeployConfig>(
            Some(absent.clone().into_os_string()),
            &no_runfiles,
        )
        .expect_err("absent config");
        assert!(
            error.starts_with(&format!("cannot read deploy config {}", absent.display())),
            "{error}"
        );
        let error = load_deploy_config_from::<DeployConfig>(
            Some(OsString::from("_main/absent.deploy.json")),
            &no_runfiles,
        )
        .expect_err("absent rlocation");
        assert_eq!(
            error,
            "cannot resolve DX_DEPLOY_CONFIG '_main/absent.deploy.json': runfile not found"
        );
    }

    #[test]
    fn an_rlocation_resolves_through_runfiles() {
        let dir = scratch_dir();
        let path = dir.path().join("demo.deploy.json");
        std::fs::write(
            &path,
            r#"{"kind":"github","config":{"deploy_name":"d","tag":"t","asset_rlocs":[]}}"#,
        )
        .expect("write config");
        let resolve = |rloc: &str| match rloc {
            "_main/demo.deploy.json" => Ok(path.clone()),
            other => no_runfiles(other),
        };
        let config = load_deploy_config_from::<DeployConfig>(
            Some(OsString::from("_main/demo.deploy.json")),
            &resolve,
        );
        assert!(matches!(config, Ok(DeployConfig::Github(_))), "{config:?}");
    }

    #[test]
    fn loading_clears_the_variable() {
        let dir = scratch_dir();
        let path = dir.path().join("demo.deploy.json");
        std::fs::write(
            &path,
            r#"{"kind":"github","config":{"deploy_name":"d","tag":"t","asset_rlocs":[]}}"#,
        )
        .expect("write config");
        let text = path.to_string_lossy().into_owned();
        with_env(&[(DEPLOY_CONFIG_ENV, Some(text.as_str()))], || {
            let config = load_deploy_config::<DeployConfig>();
            assert!(matches!(config, Ok(DeployConfig::Github(_))), "{config:?}");
            assert_eq!(std::env::var_os(DEPLOY_CONFIG_ENV), None);
        });
        with_env(
            &[(DEPLOY_CONFIG_ENV, Some("/dx-absent/demo.deploy.json"))],
            || {
                assert!(load_deploy_config::<DeployConfig>().is_err());
                assert_eq!(std::env::var_os(DEPLOY_CONFIG_ENV), None);
            },
        );
    }

    #[test]
    fn the_tool_exits_2_without_a_config() {
        with_env(&[(DEPLOY_CONFIG_ENV, None)], || {
            assert_eq!(deploy_tool_main(&["deploy_tool".to_owned()]), 2);
        });
    }

    #[test]
    fn every_kind_dispatches_to_its_main() {
        let argv = vec!["prog".to_owned(), "out".to_owned(), "extra".to_owned()];
        let configs = [
            DeployConfig::Archive(ArchiveConfig {
                app_rloc: "a".to_owned(),
                tarball_rloc: "b".to_owned(),
                checksum_rloc: "c".to_owned(),
            }),
            DeployConfig::Github(GithubConfig {
                deploy_name: "demo".to_owned(),
                tag: "v1.0.0".to_owned(),
                asset_rlocs: vec!["a".to_owned()],
            }),
            DeployConfig::CloudflarePages(CloudflarePagesConfig {
                deploy_name: "demo".to_owned(),
                account_id: "0123456789abcdef0123456789abcdef".to_owned(),
                project: "demo".to_owned(),
                branch: "main".to_owned(),
                production_branch: "main".to_owned(),
                site_rloc: "site".to_owned(),
                wrangler_rloc: "wrangler".to_owned(),
            }),
            DeployConfig::Pypi(PypiConfig {
                wheel_rloc: "a".to_owned(),
                sdist_rloc: None,
                dist_name: "demo".to_owned(),
                repository_url: "https://pypi.invalid".to_owned(),
            }),
            DeployConfig::Npm(NpmConfig {
                feed_rloc: "a".to_owned(),
                tarball_rloc: "b".to_owned(),
            }),
            DeployConfig::Nuget(NugetConfig {
                nupkg_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package_id: String::new(),
                version: String::new(),
                source: "https://nuget.invalid".to_owned(),
            }),
            DeployConfig::Crates(CratesConfig {
                crate_rlocs: vec!["a".to_owned()],
                crate_name: "demo".to_owned(),
                version: "1.0.0".to_owned(),
            }),
            DeployConfig::Maven(MavenConfig {
                jar_rloc: "a".to_owned(),
                pom_rloc: "b".to_owned(),
                sources_rloc: None,
                javadoc_rloc: None,
                group: "g".to_owned(),
                artifact: "a".to_owned(),
                version: "1.0.0".to_owned(),
                repository_url: "https://maven.invalid".to_owned(),
            }),
            DeployConfig::Oci(OciConfig {
                image_rloc: "a".to_owned(),
                registry: "ghcr.io".to_owned(),
                repository: "demo".to_owned(),
                tag: "latest".to_owned(),
                platform: "linux/amd64".to_owned(),
            }),
            DeployConfig::Octopus(OctopusConfig {
                package_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                project: "demo".to_owned(),
                channel: "Default".to_owned(),
                version: "1.0.0".to_owned(),
                deploy_to: vec!["Production".to_owned()],
                space: String::new(),
                octopus_url: "https://octopus.invalid".to_owned(),
            }),
            DeployConfig::Promotion(PromotionConfig {
                artifact_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                from_environment: "staging".to_owned(),
                to_environment: "production".to_owned(),
                version: "1.0.0".to_owned(),
            }),
        ];
        for config in &configs {
            assert_eq!(run_deploy_config(config, &argv), 1, "{config:?}");
        }
    }
}
