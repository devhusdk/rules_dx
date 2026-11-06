use super::common::*;
use super::update_unused::{self, Missing, Unused};
use dx_adopt::dependency_sets::{self, Ecosystem, Selection};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, command_started, error_event,
    notice_event,
};
use dx_plan::args::Invocation;
use dx_update::discover::{
    Candidate, Fetch, Findings, Registry, bcr, cabal, cargo, composer, containers, cpan, github,
    go, hex, julia, maven, npm, nuget, opam, pub_dev, pypi, ruby, swiftpm,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const REGISTRY_DIR_ENV: &str = "DX_UPDATE_REGISTRY_DIR";
const OPAM_SNAPSHOT_ENV: &str = "DX_OPAM_SNAPSHOT_DIR";

pub(crate) const CODE_UPDATE_OUTDATED: &str = "update_outdated";
pub(crate) const CODE_UPDATE_OUTDATED_UNSUPPORTED: &str = "update_outdated_unsupported";
pub(crate) const CODE_UPDATE_OUTDATED_LOOKUP: &str = "update_outdated_lookup_failed";
pub(crate) const CODE_UPDATE_OUTDATED_UNKNOWN: &str = "update_outdated_unknown";
pub(crate) const CODE_UPDATE_OUTDATED_OVERRIDDEN: &str = "update_outdated_overridden";

const ACTIONS_SELECTOR: &str = "github-actions";
const BAZELVERSION_SELECTOR: &str = "bazel:.bazelversion";
const CONTAINERS_SELECTOR: &str = containers::SET;

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Builtins {
    actions: Option<Vec<String>>,
    bazelversion: bool,
    containers: Option<Vec<String>>,
}

impl Builtins {
    pub(super) fn selects_all_actions(&self) -> bool {
        self.actions.as_ref().is_some_and(Vec::is_empty)
    }

    pub(super) fn selects_all_containers(&self) -> bool {
        self.containers.as_ref().is_some_and(Vec::is_empty)
    }
}

pub(super) fn split_builtin(targets: &[String]) -> (Builtins, Vec<String>) {
    let mut builtins = Builtins::default();
    let mut rest = Vec::new();
    if targets.is_empty() {
        builtins.actions = Some(Vec::new());
        builtins.bazelversion = true;
        builtins.containers = Some(Vec::new());
        return (builtins, rest);
    }
    for target in targets {
        if target == ACTIONS_SELECTOR {
            builtins.actions = Some(Vec::new());
        } else if let Some(repo) = target
            .strip_prefix(ACTIONS_SELECTOR)
            .and_then(|r| r.strip_prefix(':'))
        {
            if let Some(only) = builtins.actions.as_mut() {
                if !only.is_empty() {
                    only.push(repo.to_owned());
                }
            } else {
                builtins.actions = Some(vec![repo.to_owned()]);
            }
        } else if target == CONTAINERS_SELECTOR {
            builtins.containers = Some(Vec::new());
        } else if let Some(name) = target
            .strip_prefix(CONTAINERS_SELECTOR)
            .and_then(|r| r.strip_prefix(':'))
        {
            if let Some(only) = builtins.containers.as_mut() {
                if !only.is_empty() {
                    only.push(name.to_owned());
                }
            } else {
                builtins.containers = Some(vec![name.to_owned()]);
            }
        } else if target == BAZELVERSION_SELECTOR {
            builtins.bazelversion = true;
        } else {
            rest.push(target.clone());
        }
    }
    (builtins, rest)
}

fn github_auth(url: &str, token: Option<&str>) -> Option<(&'static str, String)> {
    let token = token.filter(|token| !token.is_empty())?;
    url.starts_with("https://api.github.com/")
        .then(|| ("Authorization", format!("Bearer {token}")))
}

pub(super) fn registry_fetch() -> Box<dyn Fetch> {
    match std::env::var_os(REGISTRY_DIR_ENV) {
        Some(dir) => Box::new(DirFetch(PathBuf::from(dir))),
        None => Box::new(HttpFetch::new()),
    }
}

pub(crate) fn execute_update_outdated(invocation: &Invocation, env: Env<'_>) -> i32 {
    let fetch = registry_fetch();
    run(invocation, env, fetch.as_ref())
}

fn run(invocation: &Invocation, env: Env<'_>, fetch: &dyn Fetch) -> i32 {
    let Env {
        workspace,
        query_runner,
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
    let (builtins, selectors) = split_builtin(&invocation.targets);
    let only_builtins = !invocation.targets.is_empty() && selectors.is_empty();
    let resolved = if only_builtins {
        Vec::new()
    } else {
        match dependency_sets::load(workspace) {
            Ok(Some(registry)) => match dependency_sets::resolve(&registry, &selectors) {
                Ok(resolved) => resolved,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            },
            Ok(None) if selectors.is_empty() => Vec::new(),
            Ok(None) => {
                let message = super::update::no_sets_message(workspace);
                return pre_exec(invocation.output, out, err, &message);
            }
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        }
    };
    let json = invocation.output == OutputMode::Json;
    if json && let Err(exit) = emit_built(out, command_started("update", "outdated")) {
        return exit;
    }
    let Collected {
        found,
        unsupported,
        ecosystems,
        ..
    } = collect(workspace, &resolved, &builtins, false, fetch);
    let unused = update_unused::find(
        workspace,
        &resolved,
        query_runner,
        &invocation.bazel_startup_options,
    );
    let missing = update_unused::find_missing(
        workspace,
        &resolved,
        query_runner,
        &invocation.bazel_startup_options,
    );
    let findings_fail = invocation.fail_on_unused && !(unused.is_empty() && missing.is_empty());
    let exit = i32::from(!found.errors.is_empty() || findings_fail);
    if let Err(code) = if json {
        emit_json(
            out,
            &found,
            &unsupported,
            &ecosystems,
            (&unused, &missing),
            exit,
        )
    } else {
        emit_text(out, &found, &unsupported, &ecosystems, (&unused, &missing))
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

/// Finds the pinned opam-repository snapshot checkout through the pinned opam wrapper.
fn opam_snapshot_dir(workspace: &Path) -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os(OPAM_SNAPSHOT_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let mut argv = dx_process::BazelHead::default().argv("run");
    argv.extend(["@rules_dx//ocaml/tools:opam", "--", "snapshot-dir"].map(str::to_owned));
    let output = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("cannot run bazel for the opam snapshot: {error}"))?;
    if !output.status.success() {
        return Err("bazel could not provide the opam-repository snapshot".to_owned());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    match text.lines().last().map(str::trim).filter(|l| !l.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => Err("the opam wrapper printed no snapshot directory".to_owned()),
    }
}

/// What the registries said about the resolved sets.
pub(super) struct Collected {
    pub found: Findings,
    pub unsupported: Vec<String>,
    pub registries: BTreeMap<String, Registry>,
    pub ecosystems: BTreeMap<String, Ecosystem>,
}

/// Looks up every resolved set and returns sorted, unique candidates.
pub(super) fn collect(
    workspace: &Path,
    resolved: &[dependency_sets::ResolvedTarget],
    builtins: &Builtins,
    prereleases: bool,
    fetch: &dyn Fetch,
) -> Collected {
    let mut found = Findings::default();
    let mut unsupported = Vec::new();
    let mut ecosystems = BTreeMap::new();
    let mut registries = BTreeMap::new();
    run_builtins(builtins, workspace, fetch, &mut found);
    for target in resolved {
        let only: &[String] = match &target.request {
            Selection::Full => &[],
            Selection::Packages(names) => names,
        };
        let name = target.set.name.as_str();
        ecosystems.insert(name.to_owned(), target.set.ecosystem);
        match target.set.ecosystem {
            Ecosystem::Bazel => {
                registries.insert(name.to_owned(), Registry::Bcr);
                for manifest in &target.set.manifests {
                    let parsed = std::fs::read_to_string(workspace.join(manifest))
                        .map_err(|e| e.to_string())
                        .and_then(|text| {
                            dx_module_bazel::ModuleFile::parse(&text).map_err(|e| e.to_string())
                        });
                    match parsed {
                        Ok(module) => merge(
                            &mut found,
                            bcr::outdated(
                                name,
                                &module,
                                only,
                                prereleases,
                                bcr::DEFAULT_BASE,
                                fetch,
                            ),
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
                registries.insert(name.to_owned(), Registry::Crates);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    cargo::outdated(
                        name,
                        &texts,
                        only,
                        prereleases,
                        cargo::DEFAULT_BASE,
                        cargo::API_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::Go => {
                registries.insert(name.to_owned(), Registry::Go);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    go::outdated(name, &texts, only, prereleases, go::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Nuget => {
                registries.insert(name.to_owned(), Registry::Nuget);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    nuget::outdated(name, &texts, only, prereleases, nuget::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Ruby => {
                registries.insert(name.to_owned(), Registry::Rubygems);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    ruby::outdated(name, &texts, only, prereleases, ruby::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Composer => {
                registries.insert(name.to_owned(), Registry::Packagist);
                let texts = read_texts(workspace, &target.set.manifests);
                let locks: Vec<String> = target
                    .set
                    .locks
                    .iter()
                    .filter(|lock| lock.ends_with("composer.lock"))
                    .cloned()
                    .collect();
                merge(
                    &mut found,
                    composer::outdated(
                        name,
                        &texts,
                        &read_texts(workspace, &locks),
                        only,
                        prereleases,
                        composer::DEFAULT_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::Cpan => {
                registries.insert(name.to_owned(), Registry::Metacpan);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    cpan::outdated(name, &texts, only, prereleases, cpan::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Cabal => {
                registries.insert(name.to_owned(), Registry::Hackage);
                let cabal_files = cabal_files(workspace, &target.set.manifests);
                let freezes: Vec<String> = target
                    .set
                    .locks
                    .iter()
                    .filter(|lock| lock.ends_with("cabal.project.freeze"))
                    .cloned()
                    .collect();
                merge(
                    &mut found,
                    cabal::outdated(
                        name,
                        &read_texts(workspace, &cabal_files),
                        &read_texts(workspace, &freezes),
                        only,
                        cabal::DEFAULT_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::SwiftPm => {
                registries.insert(name.to_owned(), Registry::SwiftUrl);
                let texts = read_texts(workspace, &target.set.locks);
                merge(
                    &mut found,
                    swiftpm::outdated(
                        name,
                        &texts,
                        only,
                        prereleases,
                        swiftpm::DEFAULT_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::Opam => {
                registries.insert(name.to_owned(), Registry::Opam);
                let texts = read_texts(workspace, &target.set.locks);
                match opam_snapshot_dir(workspace) {
                    Ok(snapshot) => merge(
                        &mut found,
                        opam::outdated(name, &texts, only, prereleases, &snapshot),
                    ),
                    Err(reason) => found.errors.push(dx_update::discover::LookupError {
                        set: name.to_owned(),
                        package: String::new(),
                        reason,
                    }),
                }
            }
            Ecosystem::Julia => {
                registries.insert(name.to_owned(), Registry::JuliaGeneral);
                let manifests: Vec<String> = target
                    .set
                    .locks
                    .iter()
                    .filter(|lock| lock.ends_with("Manifest.toml"))
                    .cloned()
                    .collect();
                merge(
                    &mut found,
                    julia::outdated(
                        name,
                        &read_texts(workspace, &target.set.manifests),
                        &read_texts(workspace, &manifests),
                        only,
                        prereleases,
                        julia::DEFAULT_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::Pub => {
                registries.insert(name.to_owned(), Registry::PubDev);
                let texts = read_texts(workspace, &target.set.locks);
                merge(
                    &mut found,
                    pub_dev::outdated(
                        name,
                        &texts,
                        only,
                        prereleases,
                        pub_dev::DEFAULT_BASE,
                        fetch,
                    ),
                );
            }
            Ecosystem::Hex => {
                registries.insert(name.to_owned(), Registry::Hex);
                let texts = read_texts(workspace, &target.set.locks);
                merge(
                    &mut found,
                    hex::outdated(name, &texts, only, prereleases, hex::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Maven => {
                registries.insert(name.to_owned(), Registry::Maven);
                let resolved: BTreeMap<String, String> = read_texts(workspace, &target.set.locks)
                    .iter()
                    .filter_map(|text| dx_audit::locks::parse_maven_install(text).ok())
                    .flatten()
                    .map(|package| (package.name, package.version))
                    .collect();
                for manifest in &target.set.manifests {
                    let parsed = std::fs::read_to_string(workspace.join(manifest))
                        .map_err(|e| e.to_string())
                        .and_then(|text| {
                            dx_module_bazel::ModuleFile::parse(&text).map_err(|e| e.to_string())
                        });
                    match parsed {
                        Ok(module) => merge(
                            &mut found,
                            maven::outdated(
                                name,
                                &module,
                                &resolved,
                                only,
                                prereleases,
                                maven::DEFAULT_BASE,
                                fetch,
                            ),
                        ),
                        Err(reason) => found.errors.push(dx_update::discover::LookupError {
                            set: name.to_owned(),
                            package: manifest.clone(),
                            reason,
                        }),
                    }
                }
            }
            Ecosystem::Npm => {
                registries.insert(name.to_owned(), Registry::Npm);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    npm::outdated(name, &texts, only, prereleases, npm::DEFAULT_BASE, fetch),
                );
            }
            Ecosystem::Uv => {
                registries.insert(name.to_owned(), Registry::Pypi);
                let texts = read_texts(workspace, &target.set.manifests);
                merge(
                    &mut found,
                    pypi::outdated(name, &texts, only, prereleases, pypi::DEFAULT_BASE, fetch),
                );
            }
            _ => unsupported.push(name.to_owned()),
        }
    }
    found
        .candidates
        .sort_by(|a, b| (&a.set, &a.package, &a.current).cmp(&(&b.set, &b.package, &b.current)));
    found.candidates.dedup();
    Collected {
        found,
        unsupported,
        registries,
        ecosystems,
    }
}

/// The registry page for a candidate, including the built-in items.
pub(super) fn registry_for(
    registries: &BTreeMap<String, Registry>,
    candidate: &Candidate,
) -> Option<Registry> {
    if candidate.set == dx_update::discover::github::ACTIONS_SET {
        return Some(Registry::GitHub);
    }
    if candidate.set == "bazel" && candidate.package == ".bazelversion" {
        return Some(Registry::BazelReleases);
    }
    registries.get(&candidate.set).copied()
}

/// The `.cabal` files next to each `cabal.project`.
pub(super) fn cabal_files(workspace: &Path, projects: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    for project in projects {
        let dir = project.rsplit_once('/').map_or("", |(dir, _)| dir);
        let Ok(entries) = std::fs::read_dir(workspace.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.ends_with(".cabal") {
                files.push(if dir.is_empty() {
                    file
                } else {
                    format!("{dir}/{file}")
                });
            }
        }
    }
    files.sort();
    files
}

fn read_texts(workspace: &Path, manifests: &[String]) -> Vec<String> {
    manifests
        .iter()
        .filter_map(|manifest| std::fs::read_to_string(workspace.join(manifest)).ok())
        .collect()
}

fn run_builtins(builtins: &Builtins, workspace: &Path, fetch: &dyn Fetch, found: &mut Findings) {
    if let Some(only) = &builtins.actions {
        let mut flows = Vec::new();
        for path in dx_update::edit::gha::list_workflows(workspace).unwrap_or_default() {
            if let Ok(text) = std::fs::read_to_string(workspace.join(&path)) {
                flows.push((path, text));
            }
        }
        merge(
            found,
            github::actions(&flows, only, github::DEFAULT_BASE, fetch),
        );
    }
    if let Some(only) = &builtins.containers
        && let Ok(text) = std::fs::read_to_string(workspace.join("MODULE.bazel"))
    {
        match dx_module_bazel::ModuleFile::parse(&text) {
            Ok(module) => merge(found, containers::outdated(&module, only, fetch)),
            Err(error) => found.errors.push(dx_update::discover::LookupError {
                set: containers::SET.to_owned(),
                package: "MODULE.bazel".to_owned(),
                reason: error.to_string(),
            }),
        }
    }
    if builtins.bazelversion
        && let Ok(text) = std::fs::read_to_string(workspace.join(".bazelversion"))
    {
        merge(
            found,
            github::bazelversion(&text, github::DEFAULT_BASE, fetch),
        );
    }
}

fn merge(into: &mut Findings, from: Findings) {
    into.unknown.extend(from.unknown);
    into.candidates.extend(from.candidates);
    into.errors.extend(from.errors);
    into.overridden.extend(from.overridden);
}

fn command_for(candidate: &Candidate, _ecosystem: Ecosystem) -> Option<String> {
    Some(format!(
        "dx update {}:{} {}",
        candidate.set, candidate.package, candidate.latest
    ))
}

fn ecosystem_of(ecosystems: &BTreeMap<String, Ecosystem>, candidate: &Candidate) -> Ecosystem {
    ecosystems
        .get(&candidate.set)
        .copied()
        .unwrap_or(Ecosystem::Bazel)
}

fn emit_json(
    out: &mut dyn Write,
    found: &Findings,
    unsupported: &[String],
    ecosystems: &BTreeMap<String, Ecosystem>,
    (unused, missing): (&[Unused], &[Missing]),
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
                related_command: command_for(candidate, ecosystem_of(ecosystems, candidate)),
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
    for unknown in &found.unknown {
        emit_built(
            out,
            notice_event(&NoticeEvent {
                scope: Some(vec![format!("{}:{}", unknown.set, unknown.package)]),
                ..NoticeEvent::warning(
                    CODE_UPDATE_OUTDATED_UNKNOWN,
                    format!("{}: {}", unknown.package, unknown.reason),
                )
            }),
        )?;
    }
    for over in &found.overridden {
        emit_built(
            out,
            notice_event(&NoticeEvent {
                scope: Some(vec![format!("{}:{}", over.set, over.package)]),
                ..NoticeEvent::warning(CODE_UPDATE_OUTDATED_OVERRIDDEN, overridden_message(over))
            }),
        )?;
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
    update_unused::emit_json(out, unused)?;
    update_unused::emit_missing_json(out, missing)?;
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

fn overridden_message(over: &dx_update::discover::Overridden) -> String {
    format!(
        "{} asks for {}, but the lock resolved {}",
        over.package, over.requested, over.resolved
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

fn emit_text(
    out: &mut dyn Write,
    found: &Findings,
    unsupported: &[String],
    ecosystems: &BTreeMap<String, Ecosystem>,
    (unused, missing): (&[Unused], &[Missing]),
) -> Result<(), i32> {
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
            command_for(candidate, ecosystem_of(ecosystems, candidate))
                .unwrap_or_else(|| "edit Gemfile, then bundle lock".to_owned())
        ))?;
    }
    for set in unsupported {
        check_stdout_write(writeln!(
            out,
            "{set}: --outdated does not cover this set yet"
        ))?;
    }
    for unknown in &found.unknown {
        check_stdout_write(writeln!(
            out,
            "{}:{}: unknown ({})",
            unknown.set, unknown.package, unknown.reason
        ))?;
    }
    for over in &found.overridden {
        check_stdout_write(writeln!(
            out,
            "{}:{}: overridden ({})",
            over.set,
            over.package,
            overridden_message(over)
        ))?;
    }
    if found.candidates.is_empty()
        && found.errors.is_empty()
        && found.unknown.is_empty()
        && found.overridden.is_empty()
    {
        check_stdout_write(writeln!(out, "nothing is outdated"))?;
    }
    update_unused::emit_text(out, unused)?;
    update_unused::emit_missing_text(out, missing)
}

pub(super) struct DirFetch(pub(super) PathBuf);

impl Fetch for DirFetch {
    fn digest(&self, url: &str) -> Result<String, String> {
        self.get(url).map(|text| text.trim().to_owned())
    }

    fn get(&self, url: &str) -> Result<String, String> {
        let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
        let path: &Path = &self.0.join(rest);
        std::fs::read_to_string(path).map_err(|_| "HTTP 404".to_owned())
    }
}

struct HttpFetch(ureq::Agent, Option<String>);

const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";

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
        let token = ["GH_TOKEN", "GITHUB_TOKEN"]
            .iter()
            .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
        HttpFetch(agent, token)
    }
}

impl Fetch for HttpFetch {
    fn digest(&self, url: &str) -> Result<String, String> {
        let response = self
            .0
            .head(url)
            .header("Accept", MANIFEST_ACCEPT)
            .call()
            .map_err(|e| e.to_string())?;
        response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| "no Docker-Content-Digest header".to_owned())
    }

    fn get(&self, url: &str) -> Result<String, String> {
        let mut request = self.0.get(url);
        if let Some((name, value)) = github_auth(url, self.1.as_deref()) {
            request = request
                .header(name, value)
                .header("Accept", "application/vnd.github+json");
        }
        let mut response = request.call().map_err(|e| e.to_string())?;
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
    fn registry_for_picks_builtins_before_the_set_map() {
        let cand = |set: &str, package: &str| Candidate {
            set: set.into(),
            package: package.into(),
            current: "1".into(),
            latest: "2".into(),
            published: None,
            prerelease: false,
            repo: None,
            tags: None,
        };
        let mut map = BTreeMap::new();
        map.insert("bazel".to_owned(), Registry::Bcr);
        assert_eq!(
            registry_for(&map, &cand("github-actions", "actions/checkout")),
            Some(Registry::GitHub)
        );
        assert_eq!(
            registry_for(&map, &cand("bazel", ".bazelversion")),
            Some(Registry::BazelReleases)
        );
        assert_eq!(
            registry_for(&map, &cand("bazel", "rules_go")),
            Some(Registry::Bcr)
        );
        assert_eq!(registry_for(&map, &cand("other", "x")), None);
    }

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
            prerelease: false,
            repo: None,
            tags: None,
        };
        assert_eq!(
            command_for(&candidate, Ecosystem::Cargo),
            Some("dx update cargo:serde 2.0.0".to_owned())
        );
    }

    #[test]
    fn text_output_lists_candidates_or_says_nothing() {
        let mut out = Vec::new();
        emit_text(
            &mut out,
            &Findings::default(),
            &[],
            &BTreeMap::new(),
            (&[], &[]),
        )
        .expect("text");
        assert_eq!(String::from_utf8_lossy(&out), "nothing is outdated\n");
        let found = Findings {
            candidates: vec![Candidate {
                set: "cargo".into(),
                package: "serde".into(),
                current: "1".into(),
                latest: "2.0.0".into(),
                published: Some("2026-01-01T00:00:00Z".into()),
                prerelease: false,
                repo: None,
                tags: None,
            }],
            ..Findings::default()
        };
        let mut out = Vec::new();
        emit_text(
            &mut out,
            &found,
            &["gradle".into()],
            &BTreeMap::new(),
            (&[], &[]),
        )
        .expect("text");
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains(
            "cargo:serde 1 -> 2.0.0 (published 2026-01-01T00:00:00Z): dx update cargo:serde 2.0.0"
        ));
        assert!(text.contains("gradle: --outdated does not cover"));
    }

    #[test]
    fn builtin_selectors_split_from_sets() {
        let list = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let (all, rest) = split_builtin(&[]);
        assert!(all.actions.is_some() && all.bazelversion && rest.is_empty());
        let (one, rest) = split_builtin(&list(&["github-actions:o/r", "dx_crates"]));
        assert_eq!(one.actions, Some(list(&["o/r"])));
        assert!(!one.bazelversion);
        assert_eq!(rest, list(&["dx_crates"]));
        let (bazel, rest) = split_builtin(&list(&["bazel:.bazelversion", "github-actions"]));
        assert!(bazel.bazelversion && rest.is_empty());
        assert_eq!(bazel.actions, Some(Vec::new()));
    }

    #[test]
    fn containers_builtin_reports_a_moved_latest_digest() {
        let old = format!("sha256:{}", "1".repeat(64));
        let new = format!("sha256:{}", "2".repeat(64));
        let root = std::env::temp_dir().join(format!("dx-containers-{}", std::process::id()));
        let manifest = root.join("mirror/gcr.io/v2/distroless/static-debian12/manifests");
        std::fs::create_dir_all(&manifest).expect("mirror");
        std::fs::write(manifest.join("latest"), format!("{new}\n")).expect("digest");
        std::fs::write(
            root.join("MODULE.bazel"),
            format!(
                "oci.pull(name = \"distroless_static\", image = \"gcr.io/distroless/static-debian12\", digest = \"{old}\")\n"
            ),
        )
        .expect("module");
        let (builtins, rest) = split_builtin(&["containers".to_owned()]);
        assert!(builtins.selects_all_containers() && rest.is_empty());
        let fetch = DirFetch(root.join("mirror"));
        let collected = collect(&root, &[], &builtins, false, &fetch);
        let _ = std::fs::remove_dir_all(&root);
        let candidate = &collected.found.candidates[0];
        assert_eq!(candidate.set, "containers");
        assert_eq!(candidate.package, "distroless_static");
        assert_eq!((&candidate.current, &candidate.latest), (&old, &new));
        assert!(collected.found.errors.is_empty());
    }

    #[test]
    fn token_goes_to_api_github_only() {
        let api = "https://api.github.com/repos/o/r/releases/latest";
        assert_eq!(
            github_auth(api, Some("t")),
            Some(("Authorization", "Bearer t".to_owned()))
        );
        assert_eq!(github_auth(api, None), None);
        assert_eq!(github_auth("https://bcr.bazel.build/x", Some("t")), None);
        assert_eq!(github_auth("https://index.crates.io/x", Some("t")), None);
        assert_eq!(
            github_auth("https://api.github.com.evil.io/x", Some("t")),
            None
        );
    }

    #[test]
    fn dir_fetch_maps_github_release_urls() {
        let dir = std::env::temp_dir().join(format!("dx-outdated-gh-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("api.github.com/repos/o/r/releases")).expect("mkdir");
        std::fs::write(dir.join("api.github.com/repos/o/r/releases/latest"), "{}").expect("write");
        let fetch = DirFetch(dir.clone());
        assert_eq!(
            fetch.get("https://api.github.com/repos/o/r/releases/latest"),
            Ok("{}".into())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn text_output_shows_unknown_pins() {
        let found = Findings {
            unknown: vec![dx_update::discover::LookupError {
                set: "github-actions".into(),
                package: "o/r".into(),
                reason: "no GitHub release".into(),
            }],
            ..Findings::default()
        };
        let mut out = Vec::new();
        emit_text(&mut out, &found, &[], &BTreeMap::new(), (&[], &[])).expect("text");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "github-actions:o/r: unknown (no GitHub release)\n"
        );
    }

    #[test]
    fn an_overridden_version_warns_in_text_and_json() {
        let found = Findings {
            overridden: vec![dx_update::discover::Overridden {
                set: "maven".into(),
                package: "com.google.guava:guava".into(),
                requested: "33.4.0-jre".into(),
                resolved: "33.6.0-android".into(),
            }],
            ..Findings::default()
        };
        let mut out = Vec::new();
        emit_text(&mut out, &found, &[], &BTreeMap::new(), (&[], &[])).expect("text");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "maven:com.google.guava:guava: overridden (com.google.guava:guava asks for 33.4.0-jre, but the lock resolved 33.6.0-android)\n"
        );
        let mut out = Vec::new();
        emit_json(&mut out, &found, &[], &BTreeMap::new(), (&[], &[]), 0).expect("json");
        let json = String::from_utf8_lossy(&out);
        assert!(json.contains(CODE_UPDATE_OUTDATED_OVERRIDDEN), "{json}");
        assert!(json.contains("\"maven:com.google.guava:guava\""), "{json}");
    }
}
