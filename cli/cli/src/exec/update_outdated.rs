use super::common::*;
use dx_adopt::dependency_sets::{self, Ecosystem, Selection};
use dx_output::{
    command_finished, command_started, error_event, notice_event, FinishedCounts, NoticeEvent,
    OutputMode,
};
use dx_plan::args::Invocation;
use dx_update::discover::{bcr, cargo, npm, pypi, Candidate, Fetch, Findings};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const REGISTRY_DIR_ENV: &str = "DX_UPDATE_REGISTRY_DIR";

pub(crate) const CODE_UPDATE_OUTDATED: &str = "update_outdated";
pub(crate) const CODE_UPDATE_OUTDATED_UNSUPPORTED: &str = "update_outdated_unsupported";
pub(crate) const CODE_UPDATE_OUTDATED_LOOKUP: &str = "update_outdated_lookup_failed";

pub(crate) fn execute_update_outdated(invocation: &Invocation, env: Env<'_>) -> i32 {
    let fetch: Box<dyn Fetch> = match std::env::var_os(REGISTRY_DIR_ENV) {
        Some(dir) => Box::new(DirFetch(PathBuf::from(dir))),
        None => Box::new(HttpFetch::new()),
    };
    run(invocation, env, fetch.as_ref())
}

fn run(invocation: &Invocation, env: Env<'_>, fetch: &dyn Fetch) -> i32 {
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    if invocation.offline {
        return operational(
            invocation,
            out,
            err,
            CODE_OFFLINE_REQUIRED,
            "dx update --outdated reads package registries; drop --offline",
        );
    }
    let registry = match dependency_sets::load(workspace) {
        Ok(Some(registry)) => registry,
        Ok(None) => return pre_exec(invocation.output, out, err, "no dependency sets"),
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let resolved = match dependency_sets::resolve(&registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let json = invocation.output == OutputMode::Json;
    if json {
        if let Err(exit) = emit_built(out, command_started("update", "outdated")) {
            return exit;
        }
    }
    let mut found = Findings::default();
    let mut unsupported = Vec::new();
    for target in &resolved {
        let only: &[String] = match &target.request {
            Selection::Full => &[],
            Selection::Packages(names) => names,
        };
        let name = target.set.name.as_str();
        match target.set.ecosystem {
            Ecosystem::Bazel => {
                for manifest in &target.set.manifests {
                    let parsed = std::fs::read_to_string(workspace.join(manifest))
                        .map_err(|e| e.to_string())
                        .and_then(|text| {
                            dx_module_bazel::ModuleFile::parse(&text).map_err(|e| e.to_string())
                        });
                    match parsed {
                        Ok(module) => merge(
                            &mut found,
                            bcr::outdated(name, &module, only, bcr::DEFAULT_BASE, fetch),
                        ),
                        Err(reason) => found.errors.push(dx_update::discover::LookupError {
                            set: name.to_owned(),
                            package: manifest.clone(),
                            reason,
                        }),
                    }
                }
            }
            Ecosystem::Cargo => {
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    cargo::outdated(name, &texts, only, cargo::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Npm => {
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    npm::outdated(name, &texts, only, npm::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Uv => {
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    pypi::outdated(name, &texts, only, pypi::DEFAULT_BASE, fetch),
                );
            }
            _ => unsupported.push(name.to_owned()),
        }
    }
    found
        .candidates
        .sort_by(|a, b| (&a.set, &a.package, &a.current).cmp(&(&b.set, &b.package, &b.current)));
    found.candidates.dedup();
    let exit = i32::from(!found.errors.is_empty());
    if let Err(code) = if json {
        emit_json(out, &found, &unsupported, exit)
    } else {
        emit_text(out, &found, &unsupported)
    } {
        return code;
    }
    for error in &found.errors {
        let _ = writeln!(
            err,
            "dx: {CODE_UPDATE_OUTDATED_LOOKUP}: {}:{} ({})",
            error.set, error.package, error.reason
        );
    }
    exit
}

fn read_texts(workspace: &Path, manifests: &[String]) -> Vec<String> {
    manifests
        .iter()
        .filter_map(|manifest| std::fs::read_to_string(workspace.join(manifest)).ok())
        .collect()
}

fn merge(into: &mut Findings, from: Findings) {
    into.candidates.extend(from.candidates);
    into.errors.extend(from.errors);
}

fn command_for(candidate: &Candidate) -> String {
    format!(
        "dx update {}:{} {}",
        candidate.set, candidate.package, candidate.latest
    )
}

fn emit_json(
    out: &mut dyn Write,
    found: &Findings,
    unsupported: &[String],
    exit: i32,
) -> Result<(), i32> {
    for candidate in &found.candidates {
        let scope = format!("{}:{}", candidate.set, candidate.package);
        let published = candidate
            .published
            .as_ref()
            .map(|time| format!(", published {time}"))
            .unwrap_or_default();
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some(command_for(candidate)),
                scope: Some(vec![scope]),
                ..NoticeEvent::info(
                    CODE_UPDATE_OUTDATED,
                    format!(
                        "{} is at {}, newest is {}{published}",
                        candidate.package, candidate.current, candidate.latest
                    ),
                )
            }),
        )?;
    }
    for set in unsupported {
        emit_built(out, unsupported_notice(set))?;
    }
    for error in &found.errors {
        emit_built(
            out,
            error_event(
                CODE_UPDATE_OUTDATED_LOOKUP,
                &format!("{}:{}: {}", error.set, error.package, error.reason),
                None,
                None,
                Some("execute"),
            ),
        )?;
    }
    emit_built(
        out,
        Ok(command_finished(
            exit,
            &FinishedCounts {
                results_complete: Some(exit == 0),
                ..FinishedCounts::default()
            },
        )),
    )
}

fn unsupported_notice(set: &str) -> Result<serde_json::Value, dx_output::OutputError> {
    notice_event(&NoticeEvent {
        related_command: Some("dx update".to_owned()),
        scope: Some(vec![set.to_owned()]),
        ..NoticeEvent::warning(
            CODE_UPDATE_OUTDATED_UNSUPPORTED,
            format!("--outdated does not cover {set} yet"),
        )
    })
}

fn emit_text(out: &mut dyn Write, found: &Findings, unsupported: &[String]) -> Result<(), i32> {
    for candidate in &found.candidates {
        let published = candidate
            .published
            .as_ref()
            .map(|time| format!(" (published {time})"))
            .unwrap_or_default();
        check_stdout_write(writeln!(
            out,
            "{}:{} {} -> {}{published}: {}",
            candidate.set,
            candidate.package,
            candidate.current,
            candidate.latest,
            command_for(candidate)
        ))?;
    }
    for set in unsupported {
        check_stdout_write(writeln!(
            out,
            "{set}: --outdated does not cover this set yet"
        ))?;
    }
    if found.candidates.is_empty() && found.errors.is_empty() {
        check_stdout_write(writeln!(out, "nothing is outdated"))?;
    }
    Ok(())
}

struct DirFetch(PathBuf);

impl Fetch for DirFetch {
    fn get(&self, url: &str) -> Result<String, String> {
        let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
        let path: &Path = &self.0.join(rest);
        std::fs::read_to_string(path).map_err(|_| "HTTP 404".to_owned())
    }
}

struct HttpFetch(ureq::Agent);

impl HttpFetch {
    fn new() -> HttpFetch {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .https_only(true)
            .tls_config(tls)
            .proxy(ureq::Proxy::try_from_env())
            .user_agent(concat!("rules_dx/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .into();
        HttpFetch(agent)
    }
}

impl Fetch for HttpFetch {
    fn get(&self, url: &str) -> Result<String, String> {
        let mut response = self.0.get(url).call().map_err(|e| e.to_string())?;
        if response.status().as_u16() != 200 {
            return Err(format!("HTTP {}", response.status().as_u16()));
        }
        response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_fetch_maps_urls_to_mirrored_files() {
        let dir = std::env::temp_dir().join(format!("dx-outdated-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("index.crates.io/3/o")).expect("mkdir");
        std::fs::write(dir.join("index.crates.io/3/o/old"), "body").expect("write");
        let fetch = DirFetch(dir.clone());
        assert_eq!(
            fetch.get("https://index.crates.io/3/o/old"),
            Ok("body".into())
        );
        assert_eq!(
            fetch.get("https://index.crates.io/3/o/none"),
            Err("HTTP 404".into())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_fetch_maps_scoped_npm_urls() {
        let dir = std::env::temp_dir().join(format!("dx-outdated-npm-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("registry.npmjs.org/@scope")).expect("mkdir");
        std::fs::write(dir.join("registry.npmjs.org/@scope/pkg"), "{}").expect("write");
        let fetch = DirFetch(dir.clone());
        assert_eq!(
            fetch.get("https://registry.npmjs.org/@scope/pkg"),
            Ok("{}".into())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn commands_name_the_set_package_and_version() {
        let candidate = Candidate {
            set: "cargo".into(),
            package: "serde".into(),
            current: "1".into(),
            latest: "2.0.0".into(),
            published: None,
        };
        assert_eq!(command_for(&candidate), "dx update cargo:serde 2.0.0");
    }

    #[test]
    fn text_output_lists_candidates_or_says_nothing() {
        let mut out = Vec::new();
        emit_text(&mut out, &Findings::default(), &[]).expect("text");
        assert_eq!(String::from_utf8_lossy(&out), "nothing is outdated\n");
        let found = Findings {
            candidates: vec![Candidate {
                set: "cargo".into(),
                package: "serde".into(),
                current: "1".into(),
                latest: "2.0.0".into(),
                published: Some("2026-01-01T00:00:00Z".into()),
            }],
            errors: Vec::new(),
        };
        let mut out = Vec::new();
        emit_text(&mut out, &found, &["go".into()]).expect("text");
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains(
            "cargo:serde 1 -> 2.0.0 (published 2026-01-01T00:00:00Z): dx update cargo:serde 2.0.0"
        ));
        assert!(text.contains("go: --outdated does not cover"));
    }
}
