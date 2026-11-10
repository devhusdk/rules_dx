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
    /// `commits`, `file` or `none`.
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub notes_rloc: String,
    /// `off`, `warn` or `fail`.
    pub api_compat: String,
    /// `registry` or `git`.
    pub api_baseline: String,
    #[serde(default)]
    pub apidiff_rloc: String,
    #[serde(default)]
    pub go_rloc: String,
    /// Workspace-relative directories of the Go modules to check.
    #[serde(default)]
    pub go_modules: Vec<String>,
}

/// Pins one Homebrew tap formula: the GitHub release config it renders from and the tap.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomebrewTapConfig {
    pub release_rloc: String,
    pub deploy_name: String,
    pub repo: String,
    pub tap: String,
    pub formula: String,
    pub binary: String,
    pub desc: String,
    pub homepage: String,
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
    pub registry_url: String,
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub api_package: String,
    #[serde(default)]
    pub api_src: String,
    #[serde(default)]
    pub griffe_rloc: String,
}

/// Pins one Helm chart push: the chart files, the pinned helm and the target.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelmChartConfig {
    pub deploy_name: String,
    pub version: String,
    pub registry: String,
    pub repository: String,
    pub helm_rloc: String,
    pub files: Vec<crate::HelmFile>,
}

/// Pins one npm publish: the packed feed and tarball.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpmConfig {
    pub feed_rloc: String,
    pub tarball_rloc: String,
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub api_extractor_rloc: String,
}

/// Pins one NuGet push: the nupkg, its expected id and version, and the feed.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NugetConfig {
    pub nupkg_rloc: String,
    pub deploy_name: String,
    pub package_id: String,
    pub version: String,
    pub registry_url: String,
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub dotnet_rloc: String,
    #[serde(default)]
    pub apicompat_rloc: String,
}

/// Pins one CPAN upload: the dist, its expected package and version, and the optional pinned perl.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpanConfig {
    pub dist_rloc: String,
    pub deploy_name: String,
    pub package: String,
    pub version: String,
    #[serde(default)]
    pub perl_rloc: String,
}

/// Pins one Conan upload: the recipe files, reference, remote and the optional pinned conan.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConanConfig {
    pub recipe_rlocs: Vec<String>,
    pub deploy_name: String,
    pub package: String,
    pub version: String,
    pub remote: String,
    pub remote_url: String,
    #[serde(default)]
    pub conan_rloc: String,
}

/// Pins one Hackage upload: the sdist and its expected package and version.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HackageConfig {
    pub sdist_rloc: String,
    pub deploy_name: String,
    pub package: String,
    pub version: String,
}

/// Pins one Packagist update: the composer.json, its expected package and the repository URL.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagistConfig {
    pub composer_rloc: String,
    pub deploy_name: String,
    pub package: String,
    pub repository_url: String,
}

/// Pins one LuaRocks upload: the rockspec and its expected package and version.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuarocksConfig {
    pub rockspec_rloc: String,
    pub deploy_name: String,
    pub package: String,
    pub version: String,
}

/// Pins one pub.dev publish: the package files, name, version and server.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PubConfig {
    pub package_rlocs: Vec<String>,
    pub package_name: String,
    pub version: String,
    pub registry_url: String,
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub dart_rloc: String,
    #[serde(default)]
    pub apitool_rloc: String,
    #[serde(default)]
    pub flutter_dart_rloc: String,
    #[serde(default)]
    pub flutter_root_rloc: String,
    #[serde(default)]
    pub package_dir: String,
}

/// Pins one crates.io publish: the crate files, name and version.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CratesConfig {
    pub crate_rlocs: Vec<String>,
    pub crate_name: String,
    pub version: String,
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub semver_checks_rloc: String,
    #[serde(default)]
    pub crate_dir: String,
}

/// Pins one Hex publish: the package files, name, version and build tool.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HexConfig {
    pub package_rlocs: Vec<String>,
    pub package_name: String,
    pub version: String,
    pub tool: String,
}

/// Pins one RubyGems push: the gem source files, name and version.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RubygemsConfig {
    pub gem_rlocs: Vec<String>,
    pub gem_name: String,
    pub version: String,
    /// Runfile of the pinned `ruby`; empty takes `gem` from PATH.
    #[serde(default)]
    pub ruby_rloc: String,
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
    pub api_compat: String,
    pub api_baseline: String,
    #[serde(default)]
    pub japicmp_rloc: String,
    #[serde(default)]
    pub java_rloc: String,
}

/// Pins one OCI push: the image and its destination.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OciConfig {
    pub image_rloc: String,
    #[serde(default)]
    pub label: String,
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub platform: String,
    #[serde(default = "default_scan")]
    pub scan: String,
    #[serde(default = "default_scan_severity")]
    pub scan_severity: String,
    #[serde(default)]
    pub sbom_rloc: String,
}

fn default_scan() -> String {
    "fail".to_owned()
}

fn default_scan_severity() -> String {
    "HIGH".to_owned()
}

/// Describes the SBOM kept with an OCI deployment, with its package count.
fn oci_sbom_line(rloc: &str) -> String {
    let base = rloc.rsplit('/').next().unwrap_or(rloc);
    let count = crate::stage::resolve_runfile(rloc)
        .ok()
        .and_then(|path| dx_sbom::spdx_package_count(&path));
    match count {
        Some(count) => format!("  sbom: {base} {count} packages"),
        None => format!("  sbom: {base} is not a readable SPDX document"),
    }
}

/// Pins one Octopus release: the package, project and target environments.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OctopusConfig {
    pub package_rloc: String,
    pub package: String,
    pub deploy_name: String,
    pub project: String,
    pub channel: String,
    pub version: String,
    pub deploy_to: Vec<String>,
    pub space: String,
    pub registry_url: String,
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
    HomebrewTap(HomebrewTapConfig),
    CloudflarePages(CloudflarePagesConfig),
    Pypi(PypiConfig),
    HelmChart(HelmChartConfig),
    Npm(NpmConfig),
    Nuget(NugetConfig),
    Cpan(CpanConfig),
    Conan(ConanConfig),
    Hackage(HackageConfig),
    Luarocks(LuarocksConfig),
    Packagist(PackagistConfig),
    Crates(CratesConfig),
    Pub(PubConfig),
    Hex(HexConfig),
    Rubygems(RubygemsConfig),
    Maven(MavenConfig),
    Oci(OciConfig),
    Octopus(OctopusConfig),
}

/// Reads the deploy config named by `DX_DEPLOY_CONFIG`, then clears the variable.
///
/// Clearing it keeps child processes from inheriting it.
pub fn load_deploy_config<T: DeserializeOwned>() -> Result<T, String> {
    let raw = std::env::var_os(DEPLOY_CONFIG_ENV);
    // SAFETY: the process is single-threaded here or the env lock is held.
    unsafe { std::env::remove_var(DEPLOY_CONFIG_ENV) };
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
        DeployConfig::Github(github) => crate::github::github_main_checked(
            &github.asset_rlocs,
            &github.deploy_name,
            &github.tag,
            &crate::github::Notes::from_config(&github.notes, &github.notes_rloc),
            &crate::github::GoApi {
                mode: &github.api_compat,
                baseline: &github.api_baseline,
                apidiff_rloc: &github.apidiff_rloc,
                go_rloc: &github.go_rloc,
                modules: &github.go_modules,
            },
            argv,
        ),
        DeployConfig::HomebrewTap(tap) => crate::homebrew_tap_main(
            &crate::HomebrewLaunch {
                release_rloc: &tap.release_rloc,
                deploy_name: &tap.deploy_name,
                repo: &tap.repo,
                tap: &tap.tap,
                formula: &tap.formula,
                binary: &tap.binary,
                desc: &tap.desc,
                homepage: &tap.homepage,
            },
            argv,
        ),
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
        DeployConfig::Pypi(pypi) => crate::pypi_main_checked(
            &pypi.wheel_rloc,
            pypi.sdist_rloc.as_deref().unwrap_or(""),
            &pypi.dist_name,
            &pypi.registry_url,
            &crate::PypiApiCheck {
                mode: &pypi.api_compat,
                baseline: &pypi.api_baseline,
                package: &pypi.api_package,
                src: &pypi.api_src,
                griffe_rloc: &pypi.griffe_rloc,
            },
            argv,
        ),
        DeployConfig::HelmChart(helm) => crate::helm_main(
            &crate::HelmLaunch {
                deploy_name: &helm.deploy_name,
                version: &helm.version,
                registry: &helm.registry,
                repository: &helm.repository,
                helm_rloc: &helm.helm_rloc,
                files: &helm.files,
            },
            argv,
        ),
        DeployConfig::Npm(npm) => crate::npm_deploy_main_checked(
            argv,
            &npm.feed_rloc,
            &npm.tarball_rloc,
            &crate::NpmApiSettings {
                api_compat: &npm.api_compat,
                api_baseline: &npm.api_baseline,
                api_extractor_rloc: &npm.api_extractor_rloc,
            },
        ),
        DeployConfig::Nuget(nuget) => crate::nuget_main_checked(
            &nuget.nupkg_rloc,
            &nuget.deploy_name,
            &nuget.package_id,
            &nuget.version,
            &nuget.registry_url,
            &crate::NugetApiCheck {
                mode: &nuget.api_compat,
                baseline: &nuget.api_baseline,
                dotnet_rloc: &nuget.dotnet_rloc,
                apicompat_rloc: &nuget.apicompat_rloc,
            },
            argv,
        ),
        DeployConfig::Cpan(cpan) => crate::cpan_main(
            &cpan.dist_rloc,
            &cpan.deploy_name,
            &cpan.package,
            &cpan.version,
            &cpan.perl_rloc,
            argv,
        ),
        DeployConfig::Conan(conan) => crate::conan_main(
            &crate::ConanLaunch {
                recipe_rlocs: &conan.recipe_rlocs,
                deploy_name: &conan.deploy_name,
                package: &conan.package,
                version: &conan.version,
                remote: &conan.remote,
                remote_url: &conan.remote_url,
                conan_rloc: &conan.conan_rloc,
            },
            argv,
        ),
        DeployConfig::Hackage(hackage) => crate::hackage_main(
            &hackage.sdist_rloc,
            &hackage.deploy_name,
            &hackage.package,
            &hackage.version,
            argv,
        ),
        DeployConfig::Packagist(packagist) => crate::packagist_main(
            &packagist.composer_rloc,
            &packagist.deploy_name,
            &packagist.package,
            &packagist.repository_url,
            argv,
        ),
        DeployConfig::Luarocks(luarocks) => crate::luarocks_main(
            &luarocks.rockspec_rloc,
            &luarocks.deploy_name,
            &luarocks.package,
            &luarocks.version,
            argv,
        ),
        DeployConfig::Crates(crates) => crate::crates_main_checked(
            &crates.crate_rlocs,
            &crates.crate_name,
            &crates.version,
            &crate::CratesApiCheck {
                api_compat: &crates.api_compat,
                api_baseline: &crates.api_baseline,
                semver_checks_rloc: &crates.semver_checks_rloc,
                crate_dir: &crates.crate_dir,
            },
            argv,
        ),
        DeployConfig::Pub(pub_config) => crate::pub_main_checked(
            &pub_config.package_rlocs,
            &pub_config.package_name,
            &pub_config.version,
            &pub_config.registry_url,
            &crate::PubApiCheck {
                mode: &pub_config.api_compat,
                baseline: &pub_config.api_baseline,
                dart_rloc: &pub_config.dart_rloc,
                apitool_rloc: &pub_config.apitool_rloc,
                flutter_dart_rloc: &pub_config.flutter_dart_rloc,
                flutter_root_rloc: &pub_config.flutter_root_rloc,
                package_dir: &pub_config.package_dir,
            },
            argv,
        ),
        DeployConfig::Hex(hex) => crate::hex_main(
            &hex.package_rlocs,
            &hex.package_name,
            &hex.version,
            &hex.tool,
            argv,
        ),
        DeployConfig::Rubygems(rubygems) => crate::rubygems_main(
            &rubygems.gem_rlocs,
            &rubygems.gem_name,
            &rubygems.version,
            &rubygems.ruby_rloc,
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
                api_compat: &maven.api_compat,
                api_baseline: &maven.api_baseline,
                japicmp_rloc: &maven.japicmp_rloc,
                java_rloc: &maven.java_rloc,
            },
            argv,
        ),
        DeployConfig::Oci(oci) => {
            let Some(scan) = crate::ScanMode::parse(&oci.scan) else {
                eprintln!("oci_deploy: scan '{}': want off, warn or fail", oci.scan);
                return 2;
            };
            let code = crate::oci_main(
                &oci.image_rloc,
                &oci.registry,
                &oci.repository,
                &oci.tag,
                &oci.platform,
                crate::OciChecks {
                    scan,
                    scan_severity: oci.scan_severity.clone(),
                    label: oci.label.clone(),
                },
                argv,
            );
            if code == 0 && !oci.sbom_rloc.is_empty() {
                println!("{}", oci_sbom_line(&oci.sbom_rloc));
            }
            code
        }
        DeployConfig::Octopus(octopus) => crate::octopus_main(
            &crate::OctopusLaunch {
                package_rloc: &octopus.package_rloc,
                package: &octopus.package,
                deploy_name: &octopus.deploy_name,
                project: &octopus.project,
                channel: &octopus.channel,
                version: &octopus.version,
                deploy_to: &octopus.deploy_to,
                space_default: &octopus.space,
                url_default: &octopus.registry_url,
            },
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
    fn an_archive_config_rejects_a_size_budget() {
        let config = parse(
            r#"{"kind":"archive","config":{"app_rloc":"_main/a.txt","tarball_rloc":"_main/a.tar.gz","checksum_rloc":"_main/a.tar.gz.sha256","size_budget":"25MB"}}"#,
        );
        assert!(config.is_err());
    }

    #[test]
    fn an_oci_sbom_line_names_an_unreadable_document() {
        assert_eq!(
            oci_sbom_line("_main/none/s.spdx.json"),
            "  sbom: s.spdx.json is not a readable SPDX document"
        );
    }

    #[test]
    fn an_oci_config_carries_the_sbom() {
        let config = parse(
            r#"{"kind":"oci","config":{"image_rloc":"i","registry":"r","repository":"p","tag":"t","platform":"linux/amd64","sbom_rloc":"_main/s.spdx.json"}}"#,
        );
        assert_eq!(
            config,
            Ok(DeployConfig::Oci(OciConfig {
                image_rloc: "i".to_owned(),
                label: String::new(),
                registry: "r".to_owned(),
                repository: "p".to_owned(),
                tag: "t".to_owned(),
                platform: "linux/amd64".to_owned(),
                scan: "fail".to_owned(),
                scan_severity: "HIGH".to_owned(),
                sbom_rloc: "_main/s.spdx.json".to_owned(),
            }))
        );
    }

    #[test]
    fn a_github_config_round_trips() {
        let config = parse(
            r#"{"config":{"deploy_name":"demo","tag":"v1.0.0","asset_rlocs":["_main/a","x;y"],"api_compat":"off","api_baseline":"registry"},"kind":"github"}"#,
        );
        assert_eq!(
            config,
            Ok(DeployConfig::Github(GithubConfig {
                deploy_name: "demo".to_owned(),
                tag: "v1.0.0".to_owned(),
                asset_rlocs: vec!["_main/a".to_owned(), "x;y".to_owned()],
                notes: String::new(),
                notes_rloc: String::new(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                apidiff_rloc: String::new(),
                go_rloc: String::new(),
                go_modules: Vec::new(),
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
                r#"{"kind":"pypi","config":{"wheel_rloc":"w","sdist_rloc":"s","dist_name":"d","registry_url":"https://x","api_compat":"off","api_baseline":"registry"}}"#
            ),
            Ok(DeployConfig::Pypi(PypiConfig {
                wheel_rloc: "w".to_owned(),
                sdist_rloc: Some("s".to_owned()),
                dist_name: "d".to_owned(),
                registry_url: "https://x".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                api_package: String::new(),
                api_src: String::new(),
                griffe_rloc: String::new(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"npm","config":{"feed_rloc":"f","tarball_rloc":"t","api_compat":"fail","api_baseline":"git","api_extractor_rloc":"x"}}"#
            ),
            Ok(DeployConfig::Npm(NpmConfig {
                feed_rloc: "f".to_owned(),
                tarball_rloc: "t".to_owned(),
                api_compat: "fail".to_owned(),
                api_baseline: "git".to_owned(),
                api_extractor_rloc: "x".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"nuget","config":{"nupkg_rloc":"n","deploy_name":"d","package_id":"","version":"1.0.0","registry_url":"https://x","api_compat":"off","api_baseline":"registry"}}"#
            ),
            Ok(DeployConfig::Nuget(NugetConfig {
                nupkg_rloc: "n".to_owned(),
                deploy_name: "d".to_owned(),
                package_id: String::new(),
                version: "1.0.0".to_owned(),
                registry_url: "https://x".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                dotnet_rloc: String::new(),
                apicompat_rloc: String::new(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"nuget","config":{"nupkg_rloc":"n","deploy_name":"d","package_id":"","version":"1.0.0","registry_url":"https://x","api_compat":"fail","api_baseline":"git","dotnet_rloc":"dn","apicompat_rloc":"ac"}}"#
            ),
            Ok(DeployConfig::Nuget(NugetConfig {
                nupkg_rloc: "n".to_owned(),
                deploy_name: "d".to_owned(),
                package_id: String::new(),
                version: "1.0.0".to_owned(),
                registry_url: "https://x".to_owned(),
                api_compat: "fail".to_owned(),
                api_baseline: "git".to_owned(),
                dotnet_rloc: "dn".to_owned(),
                apicompat_rloc: "ac".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"crates","config":{"crate_rlocs":["a;b","c,d"],"crate_name":"c","version":"1.0.0","api_compat":"fail","api_baseline":"registry"}}"#
            ),
            Ok(DeployConfig::Crates(CratesConfig {
                crate_rlocs: vec!["a;b".to_owned(), "c,d".to_owned()],
                crate_name: "c".to_owned(),
                version: "1.0.0".to_owned(),
                api_compat: "fail".to_owned(),
                api_baseline: "registry".to_owned(),
                crate_dir: String::new(),
                semver_checks_rloc: String::new(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"rubygems","config":{"gem_rlocs":["a","b"],"gem_name":"g","version":"1.0.0","ruby_rloc":"r/bin/ruby"}}"#
            ),
            Ok(DeployConfig::Rubygems(RubygemsConfig {
                gem_rlocs: vec!["a".to_owned(), "b".to_owned()],
                gem_name: "g".to_owned(),
                version: "1.0.0".to_owned(),
                ruby_rloc: "r/bin/ruby".to_owned(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"maven","config":{"jar_rloc":"j","pom_rloc":"p","sources_rloc":"s","group":"g","artifact":"a","version":"1","api_compat":"off","api_baseline":"registry"}}"#
            ),
            Ok(DeployConfig::Maven(MavenConfig {
                jar_rloc: "j".to_owned(),
                pom_rloc: "p".to_owned(),
                sources_rloc: Some("s".to_owned()),
                javadoc_rloc: None,
                group: "g".to_owned(),
                artifact: "a".to_owned(),
                version: "1".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                japicmp_rloc: String::new(),
                java_rloc: String::new(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"oci","config":{"image_rloc":"i","registry":"r","repository":"p","tag":"t","platform":"linux/amd64"}}"#
            ),
            Ok(DeployConfig::Oci(OciConfig {
                image_rloc: "i".to_owned(),
                label: String::new(),
                registry: "r".to_owned(),
                repository: "p".to_owned(),
                tag: "t".to_owned(),
                platform: "linux/amd64".to_owned(),
                scan: "fail".to_owned(),
                scan_severity: "HIGH".to_owned(),
                sbom_rloc: String::new(),
            }))
        );
        assert_eq!(
            parse(
                r#"{"kind":"octopus","config":{"package_rloc":"p","package":"pk","deploy_name":"d","project":"x","channel":"c","version":"1","deploy_to":["A,B","C"],"space":"","registry_url":"https://x"}}"#
            ),
            Ok(DeployConfig::Octopus(OctopusConfig {
                package_rloc: "p".to_owned(),
                package: "pk".to_owned(),
                deploy_name: "d".to_owned(),
                project: "x".to_owned(),
                channel: "c".to_owned(),
                version: "1".to_owned(),
                deploy_to: vec!["A,B".to_owned(), "C".to_owned()],
                space: String::new(),
                registry_url: "https://x".to_owned(),
            }))
        );
    }

    #[test]
    fn a_missing_optional_rloc_parses_to_none() {
        let config = parse(
            r#"{"kind":"pypi","config":{"wheel_rloc":"w","dist_name":"d","registry_url":"https://x","api_compat":"off","api_baseline":"registry"}}"#,
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
                r#"{"kind":"github","config":{"deploy_name":"a","tag":"b","asset_rlocs":[],"api_compat":"off","api_baseline":"registry"},"extra":1}"#,
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
            r#"{"kind":"github","config":{"deploy_name":"d","tag":"t","asset_rlocs":[],"api_compat":"off","api_baseline":"registry"}}"#,
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
            r#"{"kind":"github","config":{"deploy_name":"d","tag":"t","asset_rlocs":[],"api_compat":"off","api_baseline":"registry"}}"#,
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
                notes: String::new(),
                notes_rloc: String::new(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                apidiff_rloc: String::new(),
                go_rloc: String::new(),
                go_modules: Vec::new(),
            }),
            DeployConfig::HomebrewTap(HomebrewTapConfig {
                release_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                repo: "org/demo".to_owned(),
                tap: "org/homebrew-tap".to_owned(),
                formula: "demo".to_owned(),
                binary: "demo".to_owned(),
                desc: "demo".to_owned(),
                homepage: "https://example.invalid".to_owned(),
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
                registry_url: "https://pypi.invalid".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                api_package: String::new(),
                api_src: String::new(),
                griffe_rloc: String::new(),
            }),
            DeployConfig::HelmChart(HelmChartConfig {
                deploy_name: "demo".to_owned(),
                version: "1.0.0".to_owned(),
                registry: "registry.invalid".to_owned(),
                repository: String::new(),
                helm_rloc: "helm".to_owned(),
                files: Vec::new(),
            }),
            DeployConfig::Npm(NpmConfig {
                feed_rloc: "a".to_owned(),
                tarball_rloc: "b".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                api_extractor_rloc: String::new(),
            }),
            DeployConfig::Nuget(NugetConfig {
                nupkg_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package_id: String::new(),
                version: String::new(),
                registry_url: "https://nuget.invalid".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                dotnet_rloc: String::new(),
                apicompat_rloc: String::new(),
            }),
            DeployConfig::Cpan(CpanConfig {
                dist_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package: String::new(),
                version: String::new(),
                perl_rloc: String::new(),
            }),
            DeployConfig::Conan(ConanConfig {
                recipe_rlocs: vec!["a".to_owned()],
                deploy_name: "demo".to_owned(),
                package: "demo".to_owned(),
                version: String::new(),
                remote: "dx".to_owned(),
                remote_url: "https://conan.invalid".to_owned(),
                conan_rloc: String::new(),
            }),
            DeployConfig::Hackage(HackageConfig {
                sdist_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package: String::new(),
                version: String::new(),
            }),
            DeployConfig::Packagist(PackagistConfig {
                composer_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package: "acme/demo".to_owned(),
                repository_url: "https://github.com/acme/demo".to_owned(),
            }),
            DeployConfig::Luarocks(LuarocksConfig {
                rockspec_rloc: "a".to_owned(),
                deploy_name: "demo".to_owned(),
                package: String::new(),
                version: String::new(),
            }),
            DeployConfig::Crates(CratesConfig {
                crate_rlocs: vec!["a".to_owned()],
                crate_name: "demo".to_owned(),
                version: "1.0.0".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                crate_dir: String::new(),
                semver_checks_rloc: String::new(),
            }),
            DeployConfig::Rubygems(RubygemsConfig {
                gem_rlocs: vec!["a".to_owned()],
                gem_name: "demo".to_owned(),
                version: "1.0.0".to_owned(),
                ruby_rloc: String::new(),
            }),
            DeployConfig::Maven(MavenConfig {
                jar_rloc: "a".to_owned(),
                pom_rloc: "b".to_owned(),
                sources_rloc: None,
                javadoc_rloc: None,
                group: "g".to_owned(),
                artifact: "a".to_owned(),
                version: "1.0.0".to_owned(),
                api_compat: "off".to_owned(),
                api_baseline: "registry".to_owned(),
                japicmp_rloc: String::new(),
                java_rloc: String::new(),
            }),
            DeployConfig::Oci(OciConfig {
                image_rloc: "a".to_owned(),
                label: String::new(),
                registry: "ghcr.io".to_owned(),
                repository: "demo".to_owned(),
                tag: "latest".to_owned(),
                platform: "linux/amd64".to_owned(),
                scan: "fail".to_owned(),
                scan_severity: "HIGH".to_owned(),
                sbom_rloc: String::new(),
            }),
            DeployConfig::Octopus(OctopusConfig {
                package_rloc: "a".to_owned(),
                package: "demo".to_owned(),
                deploy_name: "demo".to_owned(),
                project: "demo".to_owned(),
                channel: "Default".to_owned(),
                version: "1.0.0".to_owned(),
                deploy_to: vec!["Production".to_owned()],
                space: String::new(),
                registry_url: "https://octopus.invalid".to_owned(),
            }),
        ];
        for config in &configs {
            assert_eq!(run_deploy_config(config, &argv), 1, "{config:?}");
        }
    }
}
