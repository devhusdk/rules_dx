//! Shared run layer for the deploy launchers: preamble, owner gate and child env.

use std::path::{Path, PathBuf};

use crate::{deploy_outdir, resolve_runfile};

/// Owner approval gate for one live-publish launcher.
pub(crate) struct OwnerGate {
    /// Launcher name used as the message prefix.
    pub(crate) tool: &'static str,
    /// Live action named in refusals.
    pub(crate) action: &'static str,
    /// Env vars that must be non-empty.
    pub(crate) credentials: &'static [&'static str],
    /// Env var that must be exactly 1.
    pub(crate) approved: &'static str,
    /// Env var that asks for the live path when exactly 1.
    pub(crate) live: &'static str,
}

/// Reads an env var, empty when unset.
pub(crate) fn env_or_empty(key: &str) -> String {
    std::env::var(key).unwrap_or_default()
}

/// Credential values read by a passed owner gate.
pub(crate) struct Secrets(Vec<(&'static str, String)>);

impl Secrets {
    /// Returns one credential value, empty when the gate does not name it.
    pub(crate) fn get(&self, key: &str) -> &str {
        self.0
            .iter()
            .find(|(name, _)| *name == key)
            .map_or("", |(_, value)| value.as_str())
    }
}

impl OwnerGate {
    /// True when the live path is requested (the live var is exactly 1).
    pub(crate) fn requested(&self, var: impl Fn(&str) -> String) -> bool {
        var(self.live) == "1"
    }

    /// Returns the refusal message, or None when credentials and approval are present.
    pub(crate) fn refusal(&self, var: impl Fn(&str) -> String) -> Option<String> {
        let (tool, action, approved) = (self.tool, self.action, self.approved);
        if self.credentials.iter().any(|key| var(key).is_empty()) {
            let creds = self.credentials.join(" plus ");
            return Some(format!(
                "{tool}: live {action} needs {creds} plus explicit owner approval ({approved}=1); refusing"
            ));
        }
        if var(approved) != "1" {
            return Some(format!(
                "{tool}: live {action} needs {approved}=1 (owner approval); refusing"
            ));
        }
        None
    }

    /// Returns the credentials when the gate passes, else the refusal message.
    pub(crate) fn authorize(&self, var: impl Fn(&str) -> String) -> Result<Secrets, String> {
        if let Some(error) = self.refusal(&var) {
            return Err(error);
        }
        Ok(Secrets(
            self.credentials
                .iter()
                .map(|key| (*key, var(key)))
                .collect(),
        ))
    }
}

/// Words that keep a prefix-matched env name out of a child env.
const SECRET_WORDS: [&str; 5] = ["TOKEN", "PASSWORD", "SECRET", "JWT", "KEY"];

/// True when one env name passes a keep-list, ignoring case on Windows.
fn keeps(keep: &[&str], name: &str) -> bool {
    keeps_in(keep, name, cfg!(windows))
}

/// Keep-list match; a `_`-suffixed entry is a prefix that skips secret words.
fn keeps_in(keep: &[&str], name: &str, ignore_case: bool) -> bool {
    let same = |left: &str, right: &str| {
        if ignore_case {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    };
    keep.iter().any(|entry| {
        if entry.ends_with('_') {
            let upper = name.to_ascii_uppercase();
            name.get(..entry.len())
                .is_some_and(|head| same(head, entry))
                && !SECRET_WORDS.iter().any(|word| upper.contains(word))
        } else {
            same(entry, name)
        }
    })
}

/// Filters process env to a keep-list plus extras.
fn minimal_env(keep: &[&str], extra: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    for (key, value) in std::env::vars_os() {
        let (Some(key), Some(value)) = (key.to_str(), value.to_str()) else {
            continue;
        };
        if keeps(keep, key) {
            env.insert(key.to_owned(), value.to_owned());
        }
    }
    for (key, value) in extra {
        env.insert((*key).to_owned(), (*value).to_owned());
    }
    env
}

/// Env names every publish child keeps; everything else is dropped.
pub(crate) const PUBLISH_CHILD_ENV_KEEP: &[&str] = &[
    "HOME",
    "LANG",
    "LC_ALL",
    "PATH",
    "TMPDIR",
    "USER",
    "LOGNAME",
    "SystemRoot",
    "SystemDrive",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramFiles",
    "ProgramData",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Extra env names `dotnet` needs to find its SDK and caches.
pub(crate) const DOTNET_CHILD_ENV_EXTRA: &[&str] = &[
    "DOTNET_ROOT",
    "DOTNET_CLI_HOME",
    "NUGET_PACKAGES",
    "DOTNET_CLI_TELEMETRY_OPTOUT",
    "DOTNET_NOLOGO",
];

/// Builds a child command with a cleared env, the keep-listed vars, then extras.
pub(crate) fn scrubbed_command(
    program: &Path,
    keep: &[&[&str]],
    extra: &[(&str, &str)],
) -> std::process::Command {
    let names: Vec<&str> = keep.iter().flat_map(|list| list.iter().copied()).collect();
    let mut cmd = std::process::Command::new(program);
    cmd.env_clear();
    for (key, value) in &minimal_env(&names, extra) {
        cmd.env(key, value);
    }
    cmd
}

/// Returns the DX_PROFILE the launcher runs under, `<unset>` when unset.
pub fn deploy_profile() -> String {
    std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned())
}

/// One deploy launcher run: its name and its output directory.
pub struct DeployRun {
    tool: &'static str,
    outdir: PathBuf,
}

impl DeployRun {
    /// Builds a run for one launcher and output directory.
    pub fn new(tool: &'static str, outdir: PathBuf) -> Self {
        Self { tool, outdir }
    }

    /// Checks argv, then creates the output directory; prints why and returns None on failure.
    ///
    /// `pinned` finishes the extra-arguments refusal, naming what the target pins.
    pub fn start(tool: &'static str, pinned: &str, argv: &[String]) -> Option<Self> {
        if argv.len() > 2 {
            eprintln!("{tool}: this deploy target takes at most an output directory; {pinned}");
            return None;
        }
        let outdir = deploy_outdir(argv);
        if std::fs::create_dir_all(&outdir).is_err() {
            eprintln!("{tool}: cannot create {}", outdir.display());
            return None;
        }
        Some(Self::new(tool, outdir))
    }

    /// Returns the launcher name.
    pub fn tool(&self) -> &'static str {
        self.tool
    }

    /// Returns the output directory.
    pub fn outdir(&self) -> &Path {
        &self.outdir
    }

    /// Resolves one runfile; prints `{tool}: {error}` and returns None on failure.
    pub fn resolve(&self, rloc: &str) -> Option<PathBuf> {
        match resolve_runfile(rloc) {
            Ok(path) => Some(path),
            Err(error) => {
                eprintln!("{}: {error}", self.tool);
                None
            }
        }
    }

    /// Like `resolve`, but an empty rlocation yields `Some(None)`.
    pub fn resolve_optional(&self, rloc: &str) -> Option<Option<PathBuf>> {
        if rloc.is_empty() {
            return Some(None);
        }
        self.resolve(rloc).map(Some)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_support::{env_test_in_child, scratch_dir, with_env};

    /// Writes a fake sh tool that prints its env, one name=value per line.
    fn env_dumper(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("dump-env");
        std::fs::write(&path, "#!/bin/sh\nexec env\n").expect("write dumper");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    fn child_env(cmd: &mut std::process::Command) -> Vec<String> {
        let out = cmd.output().expect("run dumper");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn scrubbed_command_drops_probe_and_keeps_entries() {
        if env_test_in_child("run::tests::scrubbed_command_drops_probe_and_keeps_entries") {
            return;
        }
        let scratch = scratch_dir();
        let tool = env_dumper(scratch.path());
        with_env(
            &[
                ("DX_RUN_PROBE", Some("leak")),
                ("DX_KEEP_EXACT", Some("exact")),
                ("DXPFX_PLAIN", Some("plain")),
                ("DXPFX_JOB_TOKEN", Some("t")),
                ("DXPFX_PASSWORD_X", Some("p")),
                ("DXPFX_SECRET", Some("s")),
                ("DXPFX_ID_JWT", Some("j")),
                ("DXPFX_api_key", Some("k")),
                ("DX_KEEP_TOKEN", Some("exact-token")),
            ],
            || {
                let env = child_env(&mut scrubbed_command(
                    &tool,
                    &[&["PATH", "DX_KEEP_EXACT", "DXPFX_", "DX_KEEP_TOKEN"]],
                    &[("DX_EXTRA", "extra")],
                ));
                assert!(!env.iter().any(|line| line.starts_with("DX_RUN_PROBE=")));
                assert!(env.contains(&"DX_KEEP_EXACT=exact".to_owned()));
                assert!(env.contains(&"DXPFX_PLAIN=plain".to_owned()));
                assert!(env.contains(&"DX_EXTRA=extra".to_owned()));
                assert!(
                    env.contains(&"DX_KEEP_TOKEN=exact-token".to_owned()),
                    "exact entries are not filtered by secret words"
                );
                for dropped in [
                    "DXPFX_JOB_TOKEN",
                    "DXPFX_PASSWORD_X",
                    "DXPFX_SECRET",
                    "DXPFX_ID_JWT",
                    "DXPFX_api_key",
                ] {
                    assert!(
                        !env.iter()
                            .any(|line| line.starts_with(&format!("{dropped}="))),
                        "{dropped} passed a prefix entry: {env:?}"
                    );
                }
            },
        );
    }

    #[test]
    fn keeps_matches_exact_and_filtered_prefix() {
        assert!(keeps(&["PATH"], "PATH"));
        assert!(!keeps(&["PATH"], "PATHEXT"));
        assert!(!keeps(&["PATH"], "PAT"));
        assert!(keeps(&["CI_"], "CI_PROJECT_URL"));
        assert!(!keeps(&["CI_"], "CI_JOB_TOKEN"));
        assert!(!keeps(&["CI_"], "CI_JOB_JWT_V2"));
        assert!(!keeps(&["GITLAB_"], "GITLAB_DEPLOY_KEY"));
        assert!(!keeps(&["GITLAB_"], "GITLAB_PASSWORD"));
        assert!(!keeps(&["GITLAB_"], "GITLAB_Secret"));
        assert!(!keeps(&["CI_"], "XCI_FOO"));
    }

    #[test]
    fn keeps_ignores_case_only_on_windows() {
        assert!(keeps_in(&["PATH"], "Path", true));
        assert!(keeps_in(&["ComSpec"], "COMSPEC", true));
        assert!(keeps_in(&["SystemRoot"], "SYSTEMROOT", true));
        assert!(keeps_in(&["CI_"], "ci_project_url", true));
        assert!(!keeps_in(&["CI_"], "ci_job_token", true));
        assert!(!keeps_in(&["PATH"], "Pathext", true));
        assert!(!keeps_in(&["PATH"], "Path", false));
        assert!(!keeps_in(&["CI_"], "ci_project_url", false));
        assert_eq!(keeps(&["PATH"], "Path"), cfg!(windows));
    }

    #[test]
    fn minimal_env_keeps_allowlist_and_applies_extras() {
        if env_test_in_child("run::tests::minimal_env_keeps_allowlist_and_applies_extras") {
            return;
        }
        with_env(
            &[
                ("DX_TEST_KEEP", Some("kept")),
                ("DX_TEST_DROP", Some("dropped")),
            ],
            || {
                let env = minimal_env(
                    &["DX_TEST_KEEP", "DX_TEST_ABSENT"],
                    &[("DX_TEST_EXTRA", "extra")],
                );
                assert_eq!(env.get("DX_TEST_KEEP").map(String::as_str), Some("kept"));
                assert_eq!(env.get("DX_TEST_EXTRA").map(String::as_str), Some("extra"));
                assert!(!env.contains_key("DX_TEST_DROP"));
                assert!(!env.contains_key("DX_TEST_ABSENT"));
            },
        );
    }

    const TEST_GATE: OwnerGate = OwnerGate {
        tool: "test_deploy",
        action: "publish",
        credentials: &["TEST_USER", "TEST_TOKEN"],
        approved: "TEST_APPROVED",
        live: "TEST_LIVE",
    };

    fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |key| {
            vars.iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
                .unwrap_or_default()
        }
    }

    #[test]
    fn authorize_matches_refusal_and_returns_credentials() {
        let cases: [&[(&str, &str)]; 4] = [
            &[],
            &[("TEST_USER", "u")],
            &[("TEST_USER", "u"), ("TEST_TOKEN", "t")],
            &[
                ("TEST_USER", "u"),
                ("TEST_TOKEN", "t"),
                ("TEST_APPROVED", "true"),
            ],
        ];
        for vars in cases {
            let refusal = TEST_GATE.refusal(lookup(vars)).expect("refused");
            match TEST_GATE.authorize(lookup(vars)) {
                Err(error) => assert_eq!(error, refusal),
                Ok(_) => panic!("authorize passed where refusal refused"),
            }
        }
        let vars = [
            ("TEST_USER", "u"),
            ("TEST_TOKEN", "t"),
            ("TEST_APPROVED", "1"),
        ];
        assert!(TEST_GATE.refusal(lookup(&vars)).is_none());
        let Ok(secrets) = TEST_GATE.authorize(lookup(&vars)) else {
            panic!("authorize refused a passing gate");
        };
        assert_eq!(secrets.get("TEST_USER"), "u");
        assert_eq!(secrets.get("TEST_TOKEN"), "t");
        assert_eq!(secrets.get("TEST_APPROVED"), "");
    }

    #[test]
    fn requested_is_true_only_for_exact_one() {
        assert!(TEST_GATE.requested(lookup(&[("TEST_LIVE", "1")])));
        for value in ["", "0", "true", "yes", " 1", "1 ", "01"] {
            assert!(
                !TEST_GATE.requested(lookup(&[("TEST_LIVE", value)])),
                "{value:?}"
            );
        }
        assert!(!TEST_GATE.requested(lookup(&[])));
    }

    #[test]
    fn deploy_run_start_rejects_extra_args_and_creates_dir() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("nested").join("out");
        let path = outdir.to_string_lossy().into_owned();
        let extra = vec!["prog".to_owned(), path.clone(), "more".to_owned()];
        assert!(DeployRun::start("test_deploy", "pinned", &extra).is_none());
        assert!(!outdir.exists());
        let run =
            DeployRun::start("test_deploy", "pinned", &["prog".to_owned(), path]).expect("started");
        assert!(outdir.is_dir());
        assert_eq!(run.outdir(), outdir);
        assert_eq!(run.tool(), "test_deploy");
        assert_eq!(run.resolve_optional("").expect("empty is fine"), None);
        assert!(run.resolve("").is_none());
    }

    #[test]
    fn deploy_profile_reports_unset() {
        if env_test_in_child("run::tests::deploy_profile_reports_unset") {
            return;
        }
        with_env(&[("DX_PROFILE", None)], || {
            assert_eq!(deploy_profile(), "<unset>");
        });
        with_env(&[("DX_PROFILE", Some("release"))], || {
            assert_eq!(deploy_profile(), "release");
        });
    }
}
