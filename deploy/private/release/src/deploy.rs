//! Deploy bodies for `bcr_check` and `signed_release`, run by `release_deploy_tool`.

use std::fmt;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use crate::{BcrError, CosignPin, bcr_run, signing_run};

/// Config for one `bcr_check` deploy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BcrDeployConfig {
    pub module: String,
    pub version: String,
    pub input_rlocs: Vec<String>,
}

/// Config for one `signed_release` deploy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningDeployConfig {
    pub identity: String,
    pub issuer: String,
    pub cosign_rloc: String,
    pub cosign_version: String,
    pub cosign_sha256: String,
    pub asset_rlocs: Vec<String>,
}

/// The deploy kinds that only `release_deploy_tool` runs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    content = "config",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReleaseDeployConfig {
    Bcr(BcrDeployConfig),
    Signing(SigningDeployConfig),
}

/// A failed `bcr_check` or `signed_release` deploy.
#[derive(Debug)]
pub enum DeployError {
    BcrArgs,
    SigningArgs,
    Runfile {
        prefix: &'static str,
        rloc: String,
        source: io::Error,
    },
    Bcr(BcrError),
    Signing(crate::SigningError),
}

impl fmt::Display for DeployError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BcrArgs => f.write_str(
                "bcr: this deploy target takes no extra args; the submission is exactly the pinned inputs",
            ),
            Self::SigningArgs => f.write_str(
                "signing: this deploy target takes at most an output directory; artifacts are pinned at analysis time",
            ),
            Self::Runfile { prefix, rloc, .. } => {
                write!(f, "{prefix}: runfile not found for '{rloc}'")
            }
            Self::Bcr(source) => source.fmt(f),
            Self::Signing(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for DeployError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runfile { source, .. } => Some(source),
            Self::Bcr(source) => Some(source),
            Self::Signing(source) => Some(source),
            Self::BcrArgs | Self::SigningArgs => None,
        }
    }
}

type Resolve<'a> = &'a dyn Fn(&str) -> io::Result<PathBuf>;

fn resolve_all(
    prefix: &'static str,
    rlocs: &[String],
    resolve: Resolve<'_>,
) -> Result<Vec<String>, DeployError> {
    rlocs
        .iter()
        .map(|rloc| {
            resolve(rloc)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|source| DeployError::Runfile {
                    prefix,
                    rloc: rloc.clone(),
                    source,
                })
        })
        .collect()
}

/// Runs the BCR shape check: no extra args, every input resolved, then `bcr_run`.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn bcr_deploy(
    config: &BcrDeployConfig,
    argv: &[String],
    dry_run: bool,
    resolve: Resolve<'_>,
) -> Result<String, DeployError> {
    if argv.len() > 1 {
        return Err(DeployError::BcrArgs);
    }
    let inputs = resolve_all("bcr", &config.input_rlocs, resolve)?;
    bcr_run(&config.module, &config.version, &inputs, dry_run).map_err(DeployError::Bcr)
}

/// Runs the signing deploy: at most an outdir arg, every rloc resolved, then `signing_run`.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn signing_deploy(
    config: &SigningDeployConfig,
    argv: &[String],
    dry_run: bool,
    resolve: Resolve<'_>,
) -> Result<String, DeployError> {
    if argv.len() > 2 {
        return Err(DeployError::SigningArgs);
    }
    let outdir = dx_deploy_tools::deploy_outdir(argv);
    let assets = resolve_all("signing", &config.asset_rlocs, resolve)?;
    let cosign = resolve_all(
        "signing",
        std::slice::from_ref(&config.cosign_rloc),
        resolve,
    )?
    .remove(0);
    let pin = CosignPin {
        path: &cosign,
        version: &config.cosign_version,
        sha256: &config.cosign_sha256,
    };
    signing_run(
        &config.identity,
        &config.issuer,
        &assets,
        dry_run,
        &pin,
        &outdir,
    )
    .map_err(DeployError::Signing)
}

fn flag(name: &str) -> bool {
    std::env::var(name).unwrap_or_default() == "1"
}

/// Runs the release deploy tool: loads the config named by `DX_DEPLOY_CONFIG`, then dispatches.
///
/// Returns 2 when the config cannot be loaded, 1 when the deploy fails.
#[must_use]
pub fn release_deploy_tool_main(argv: &[String]) -> i32 {
    let config = match dx_deploy_tools::load_deploy_config::<ReleaseDeployConfig>() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("release_deploy_tool: {error}");
            return 2;
        }
    };
    let result = match &config {
        ReleaseDeployConfig::Bcr(bcr) => bcr_deploy(
            bcr,
            argv,
            flag("BCR_DRY_RUN"),
            &dx_deploy_tools::resolve_runfile,
        ),
        ReleaseDeployConfig::Signing(signing) => signing_deploy(
            signing,
            argv,
            flag("RELEASE_SIGN_DRY_RUN"),
            &dx_deploy_tools::resolve_runfile,
        ),
    };
    match result {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BCR: &str = r#"{"kind":"bcr","config":{"module":"rules_dx","version":"1.2.3","input_rlocs":["w/a.json"]}}"#;
    const SIGNING: &str = r#"{"kind":"signing","config":{"identity":"id","issuer":"https://token.actions.githubusercontent.com","cosign_rloc":"c/cosign","cosign_version":"v2","cosign_sha256":"abc","asset_rlocs":["w/bin"]}}"#;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "matches the resolver callback signature"
    )]
    fn ok(rloc: &str) -> io::Result<PathBuf> {
        Ok(PathBuf::from(format!("/r/{rloc}")))
    }

    fn missing(rloc: &str) -> io::Result<PathBuf> {
        Err(io::Error::new(io::ErrorKind::NotFound, rloc.to_string()))
    }

    fn argv(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("a{i}")).collect()
    }

    fn bcr_cfg(version: &str) -> BcrDeployConfig {
        BcrDeployConfig {
            module: "rules_dx".into(),
            version: version.into(),
            input_rlocs: vec!["w/a.json".into(), "w/b.json".into()],
        }
    }

    fn signing_cfg() -> SigningDeployConfig {
        match serde_json::from_str::<ReleaseDeployConfig>(SIGNING) {
            Ok(ReleaseDeployConfig::Signing(cfg)) => cfg,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn configs_round_trip() {
        assert!(matches!(
            serde_json::from_str::<ReleaseDeployConfig>(BCR),
            Ok(ReleaseDeployConfig::Bcr(_))
        ));
        assert_eq!(signing_cfg().asset_rlocs, vec!["w/bin".to_string()]);
    }

    #[test]
    fn bad_configs_are_rejected() {
        let extra = BCR.replace("\"version\"", "\"extra\":1,\"version\"");
        assert!(serde_json::from_str::<ReleaseDeployConfig>(&extra).is_err());
        let kind = BCR.replace("\"bcr\"", "\"nope\"");
        assert!(serde_json::from_str::<ReleaseDeployConfig>(&kind).is_err());
        let missing = BCR.replace("\"version\":\"1.2.3\",", "");
        assert!(serde_json::from_str::<ReleaseDeployConfig>(&missing).is_err());
    }

    #[test]
    fn kinds_do_not_overlap_with_deploy_tool() {
        let error = serde_json::from_str::<dx_deploy_tools::DeployConfig>(BCR)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(error.contains("unknown variant"), "{error}");
        let github = r#"{"kind":"github","config":{}}"#;
        let error = serde_json::from_str::<ReleaseDeployConfig>(github)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(error.contains("unknown variant"), "{error}");
    }

    #[test]
    fn arg_limits() {
        let error = bcr_deploy(&bcr_cfg("1.0.0"), &argv(2), true, &ok).unwrap_err();
        assert!(error.to_string().contains("takes no extra args"), "{error}");
        assert!(matches!(error, DeployError::BcrArgs));
        let error = signing_deploy(&signing_cfg(), &argv(3), true, &ok).unwrap_err();
        assert!(
            error.to_string().contains("at most an output directory"),
            "{error}"
        );
        assert!(matches!(error, DeployError::SigningArgs));
        assert!(signing_deploy(&signing_cfg(), &argv(2), true, &ok).is_ok());
    }

    #[test]
    fn unresolved_rlocs_are_named() {
        let error = bcr_deploy(&bcr_cfg("1.0.0"), &argv(1), true, &missing).unwrap_err();
        assert_eq!(error.to_string(), "bcr: runfile not found for 'w/a.json'");
        assert!(matches!(error, DeployError::Runfile { prefix: "bcr", .. }));
        assert!(std::error::Error::source(&error).is_some());
        let only_cosign = |rloc: &str| {
            if rloc == "c/cosign" {
                missing(rloc)
            } else {
                ok(rloc)
            }
        };
        let error = signing_deploy(&signing_cfg(), &argv(1), true, &only_cosign).unwrap_err();
        assert_eq!(
            error.to_string(),
            "signing: runfile not found for 'c/cosign'"
        );
        assert!(matches!(
            error,
            DeployError::Runfile {
                prefix: "signing",
                ..
            }
        ));
    }

    #[test]
    fn signing_errors_keep_their_source() {
        #[expect(
            clippy::unnecessary_wraps,
            reason = "matches the resolver callback signature"
        )]
        fn nonexistent(rloc: &str) -> io::Result<PathBuf> {
            Ok(PathBuf::from(format!("/nonexistent/{rloc}")))
        }
        let error = signing_deploy(&signing_cfg(), &argv(1), false, &nonexistent).unwrap_err();
        assert!(
            matches!(error, DeployError::Signing(crate::SigningError::Pin(_))),
            "{error:?}"
        );
        assert!(error.to_string().contains("cannot hash cosign"), "{error}");
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn bcr_dry_run_lists_inputs() {
        let text = bcr_deploy(&bcr_cfg("1.0.0"), &argv(1), true, &ok).unwrap();
        assert!(
            text.contains("/r/w/a.json") && text.contains("/r/w/b.json"),
            "{text}"
        );
    }

    #[test]
    fn bcr_gates() {
        for version in ["1.0.0", "0.0.0"] {
            let error = bcr_deploy(&bcr_cfg(version), &argv(1), false, &ok).unwrap_err();
            assert!(
                error.to_string().contains("release-consumer.yml"),
                "{error}"
            );
            assert!(
                matches!(error, DeployError::Bcr(BcrError::SubmittedByWorkflow)),
                "{error:?}"
            );
            assert!(std::error::Error::source(&error).is_some());
        }
    }

    #[test]
    fn signing_dry_run_never_spawns_cosign() {
        let resolve = |rloc: &str| -> io::Result<PathBuf> {
            Ok(PathBuf::from(format!("/nonexistent/{rloc}")))
        };
        let text = signing_deploy(&signing_cfg(), &argv(1), true, &resolve).unwrap();
        assert!(text.contains("command: cosign sign-blob"), "{text}");
    }
}
