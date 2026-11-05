use std::path::Path;

use dx_adopt::scaffold::ScaffoldFile;
use dx_adopt::{AdoptError, Applied, FileKind, Ownership};
use dx_plan::args::Agent;
use dx_worktree::setup;

use super::{agents, claude_md, copilot_md};

const ESSENTIAL: [&str; 6] = [
    ".dx/pins",
    ".gitignore",
    ".bazelignore",
    "MODULE.bazel",
    "tools/dx",
    "tools/dx.cmd",
];

/// How init writes one planned file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Scaffold(FileKind),
    Launcher,
    Override,
    Preset,
    Hooks,
    AgentsBlock,
    ClaudeImport,
    CopilotInstructions,
}

/// Whether apply creates missing files or only refreshes existing ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Create,
    Refresh,
}

/// One file init manages.
pub(super) struct InitFile {
    pub(super) path: String,
    pub(super) content: String,
    pub(super) kind: Kind,
    pub(super) essential: bool,
}

impl InitFile {
    /// What dx does to this file once you have edited it.
    pub(super) fn ownership(&self) -> Ownership {
        match self.kind {
            Kind::Scaffold(kind) => match kind {
                FileKind::WriteOnce if self.path == "MODULE.bazel" => Ownership::Lines,
                FileKind::WriteOnce => Ownership::Once,
                FileKind::VscodeKeys | FileKind::EditorconfigKeys => Ownership::Keys,
                FileKind::VscodeRecommendations | FileKind::Bazelignore => Ownership::Lines,
                FileKind::CiPin(_) | FileKind::DevcontainerPins => Ownership::Pin,
                FileKind::Generated => Ownership::Whole,
            },
            Kind::Launcher | Kind::Override => Ownership::Whole,
            Kind::Preset if self.path == ".bazelrc" || self.path == ".gitignore" => {
                Ownership::Lines
            }
            Kind::Preset => Ownership::Whole,
            Kind::Hooks => Ownership::Keys,
            Kind::AgentsBlock => Ownership::Block,
            Kind::ClaudeImport | Kind::CopilotInstructions => Ownership::Lines,
        }
    }
}

/// The files each agent tool owns; the one place that maps an agent to its files.
fn agent_paths(agent: Agent) -> &'static [&'static str] {
    match agent {
        Agent::Claude => &[setup::SETTINGS, claude_md::FILE],
        Agent::Codex => &[setup::CODEX.path],
        Agent::Gemini => &[setup::GEMINI.path],
        Agent::Copilot => &[copilot_md::FILE, setup::COPILOT.path],
    }
}

/// Every file init manages, once each: scaffold files, launchers, overrides, preset, AGENTS.md, and the tool files that exist or whose agent is selected.
pub(super) fn plan(
    root: &Path,
    module: &str,
    fresh: bool,
    agents: &[Agent],
) -> Result<Vec<InitFile>, AdoptError> {
    plan_with_profile(root, module, fresh, agents, None)
}

/// `plan` with the config profile the root files pick; without one they keep their current pick.
pub(super) fn plan_with_profile(
    root: &Path,
    module: &str,
    fresh: bool,
    agents: &[Agent],
    profile: Option<dx_adopt::Profile>,
) -> Result<Vec<InitFile>, AdoptError> {
    let profile = Some(dx_adopt::resolve_profile(root, profile));
    let selected: Vec<&str> = agents
        .iter()
        .flat_map(|agent| agent_paths(*agent).iter().copied())
        .collect();
    let wanted = |path: &str| selected.contains(&path) || root.join(path).exists();
    let mut scaffold = if fresh {
        dx_adopt::plan_fresh_files(module)
    } else {
        Vec::new()
    };
    scaffold.extend(dx_adopt::plan_init_files_at(root, module, profile)?);
    let mut entries: Vec<(String, String, Kind)> = scaffold
        .into_iter()
        .map(|file| (file.path, file.content, Kind::Scaffold(file.kind)))
        .collect();
    entries.push((
        dx_adopt::LAUNCHER_SH_REL.to_owned(),
        dx_adopt::render_launcher_sh(),
        Kind::Launcher,
    ));
    entries.push((
        dx_adopt::LAUNCHER_CMD_REL.to_owned(),
        dx_adopt::render_launcher_cmd(),
        Kind::Launcher,
    ));
    entries.extend(
        dx_adopt::render_overrides(&dx_adopt::module_deps(root))
            .into_iter()
            .map(|(path, content)| (path, content, Kind::Override)),
    );
    entries.extend(
        dx_adopt::preset_files(root, profile)
            .into_iter()
            .map(|(path, content)| (path, content, Kind::Preset)),
    );
    entries.extend(
        setup::HOOK_FILES
            .iter()
            .filter(|file| wanted(file.path))
            .map(|file| {
                (
                    file.path.to_owned(),
                    setup::settings_text(file),
                    Kind::Hooks,
                )
            }),
    );
    entries.push((agents::FILE.to_owned(), agents::block(), Kind::AgentsBlock));
    if wanted(claude_md::FILE) {
        entries.push((
            claude_md::FILE.to_owned(),
            format!("@{}\n", agents::FILE),
            Kind::ClaudeImport,
        ));
    }
    if wanted(copilot_md::FILE) {
        entries.push((
            copilot_md::FILE.to_owned(),
            copilot_md::BLOCK.to_owned(),
            Kind::CopilotInstructions,
        ));
    }
    let overrides_included = std::fs::read_to_string(root.join("MODULE.bazel"))
        .is_ok_and(|text| text.contains(dx_adopt::OVERRIDES_INCLUDE));
    let mut files: Vec<InitFile> = Vec::new();
    for (path, content, kind) in entries {
        if files.iter().all(|file| file.path != path) {
            let essential = ESSENTIAL.contains(&path.as_str())
                || selected.contains(&path.as_str())
                || (kind == Kind::Override && overrides_included);
            files.push(InitFile {
                path,
                content,
                kind,
                essential,
            });
        }
    }
    Ok(files)
}

/// Refuses when `MODULE.bazel` asks for another rules_dx than this dx binary.
fn check_module_version(root: &Path) -> Result<(), String> {
    match dx_adopt::module_rules_dx_version(root) {
        Some(pinned)
            if !dx_adopt::version_pin_matches_module(&pinned, dx_adopt::MODULE_VERSION) =>
        {
            Err(format!(
                "MODULE.bazel pins rules_dx {pinned}, dx is {}; run `{}`",
                dx_adopt::MODULE_VERSION,
                dx_adopt::init_apply_command(root)
            ))
        }
        _ => Ok(()),
    }
}

fn check_pin(root: &Path, prefix: &str) -> Result<(), String> {
    match dx_adopt::read_version_pin(root) {
        Ok(pin) if !pin.is_empty() && !dx_adopt::pin_is_valid(&pin) => Err(adopt_message(
            prefix,
            &AdoptError::InvalidVersionPin { version: pin },
        )),
        _ => Ok(()),
    }
}

/// Prefixes an entry's path, keeping a `removed:` marker in front.
fn prefix_entry(prefix: &str, entry: &str) -> String {
    match entry.strip_prefix("removed:") {
        Some(path) => format!("removed:{prefix}{path}"),
        None => format!("{prefix}{entry}"),
    }
}

fn adopt_message(prefix: &str, error: &AdoptError) -> String {
    match error {
        AdoptError::NotADirectory { path } => format!("{prefix}{path} is not a directory"),
        _ => error.to_string(),
    }
}

fn hook_paths(files: &[InitFile]) -> Vec<&str> {
    files
        .iter()
        .filter(|file| file.kind == Kind::Hooks)
        .map(|file| file.path.as_str())
        .collect()
}

/// Fails before any write when a planned file cannot be read or written under `root`.
pub(super) fn preflight(root: &Path, prefix: &str, files: &[InitFile]) -> Result<(), String> {
    if root.exists() && !root.is_dir() {
        return Err(format!(
            "{} is not a directory",
            prefix.trim_end_matches('/')
        ));
    }
    let destinations: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    dx_adopt::preflight_destinations(root, &destinations)
        .map_err(|error| adopt_message(prefix, &error))?;
    check_pin(root, prefix)?;
    dx_adopt::plan_launcher_files(root, true).map_err(|error| adopt_message(prefix, &error))?;
    dx_adopt::preflight_overrides(root).map_err(|error| adopt_message(prefix, &error))?;
    dx_adopt::preflight_preset(root).map_err(|error| adopt_message(prefix, &error))?;
    let io = |error: std::io::Error| error.to_string();
    setup::inspect(root, &hook_paths(files)).map_err(io)?;
    agents::planned(root, true).map_err(io)?;
    claude_md::state(root).map_err(io)?;
    copilot_md::imported(root).map_err(io)?;
    Ok(())
}

/// Writes the planned files under `root`, scaffold files first, then launchers, overrides, preset, hooks, AGENTS.md and CLAUDE.md.
///
/// Records each path with `prefix` in `applied` as it lands and each advisory in `notes`; prints nothing.
pub(super) fn apply(
    root: &Path,
    prefix: &str,
    files: &[InitFile],
    mode: Mode,
    applied: &mut Applied,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    check_module_version(root)?;
    let has = |wanted: fn(&Kind) -> bool| files.iter().any(|file| wanted(&file.kind));
    if mode == Mode::Refresh {
        return refresh(root, prefix, files, applied);
    }
    let prefixed = |paths: Vec<String>| paths.into_iter().map(|path| format!("{prefix}{path}"));
    let scaffold: Vec<ScaffoldFile> = files
        .iter()
        .filter_map(|file| match file.kind {
            Kind::Scaffold(kind) => Some(ScaffoldFile {
                path: file.path.clone(),
                content: file.content.clone(),
                kind,
            }),
            _ => None,
        })
        .collect();
    let mut own = Applied::default();
    let written = dx_adopt::scaffold::apply_scaffold(root, &scaffold, &mut own);
    if std::fs::read_to_string(root.join(".bazelignore"))
        .is_ok_and(|text| !dx_adopt::bazelignore_covers_dx(&text))
    {
        notes.push(format!(
            "{prefix}.bazelignore lacks .dx/ide; add the dx lines so Bazel skips dx state"
        ));
    }
    if dx_adopt::dx_config_git_ignored(root) {
        notes.push(format!(
            "{prefix}.gitignore hides .dx/config; ignore /.dx/* instead of .dx/ and add !/.dx/config/"
        ));
    }
    applied.wrote.extend(prefixed(own.wrote));
    applied.refused.extend(prefixed(own.refused));
    written.map_err(|error| error.to_string())?;
    check_pin(root, prefix)?;
    if has(|kind| *kind == Kind::Launcher) {
        let mut launchers = Applied::default();
        let written = dx_adopt::write_launcher_files(root, &mut launchers);
        applied.wrote.extend(prefixed(launchers.wrote));
        applied.refused.extend(prefixed(launchers.refused));
        written.map_err(|error| adopt_message(prefix, &error))?;
    }
    if has(|kind| *kind == Kind::Override) {
        for entry in
            dx_adopt::apply_overrides(root).map_err(|error| adopt_message(prefix, &error))?
        {
            match entry.strip_prefix("refused:") {
                Some(refused) => notes.push(format!("{prefix}{refused}")),
                None => applied.wrote.push(prefix_entry(prefix, &entry)),
            }
        }
    }
    wire_module(root, prefix, applied, notes)?;
    if has(|kind| *kind == Kind::Preset) {
        let mut preset = Applied::default();
        let mut preset_notes = Vec::new();
        let profile = files
            .iter()
            .find(|file| file.kind == Kind::Preset && file.path.ends_with("correctness.bazelrc"))
            .map(|file| dx_adopt::layer_profile(&file.content));
        let written = dx_adopt::write_preset(root, &mut preset, &mut preset_notes, profile);
        applied.wrote.extend(prefixed(preset.wrote));
        notes.extend(
            preset_notes
                .into_iter()
                .map(|note| format!("{prefix}{note}")),
        );
        written.map_err(|error| adopt_message(prefix, &error))?;
    }
    if has(|kind| *kind == Kind::Hooks) {
        let outcome = setup::apply(root, &hook_paths(files)).map_err(|error| error.to_string())?;
        applied
            .wrote
            .extend(outcome.wrote.iter().map(|path| format!("{prefix}{path}")));
        for (path, missing) in &outcome.missing {
            notes.push(format!(
                "{prefix}{path} exists; add these hooks: {}",
                missing.join("; ")
            ));
        }
        if !outcome.missing_allow.is_empty() {
            notes.push(format!(
                "{prefix}{} exists; add these permissions.allow entries: {}",
                setup::SETTINGS,
                outcome.missing_allow.join("; ")
            ));
        }
    }
    if has(|kind| *kind == Kind::AgentsBlock)
        && agents::write(root, true).map_err(|error| error.to_string())?
    {
        applied.wrote.push(format!("{prefix}{}", agents::FILE));
    }
    if has(|kind| *kind == Kind::ClaudeImport)
        && claude_md::write(root).map_err(|error| error.to_string())?
    {
        applied.wrote.push(format!("{prefix}{}", claude_md::FILE));
    }
    if has(|kind| *kind == Kind::CopilotInstructions)
        && copilot_md::write(root).map_err(|error| error.to_string())?
    {
        applied.wrote.push(format!("{prefix}{}", copilot_md::FILE));
    }
    Ok(())
}

/// Adds the `MODULE.bazel` stanzas for the root lock files, then the rules_dx family modules,
/// reporting what it could not wire.
fn wire_module(
    root: &Path,
    prefix: &str,
    applied: &mut Applied,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let hubs = dx_adopt::plan_lock_hubs(root).map_err(|error| adopt_message(prefix, &error))?;
    notes.extend(hubs.notes.iter().map(|note| format!("{prefix}{note}")));
    if let Some(edit) = hubs.edit {
        dx_adopt::write_module_edit(root, &edit).map_err(|error| adopt_message(prefix, &error))?;
        applied.wrote.push(format!("{prefix}{}", edit.path));
    }
    let deps = dx_adopt::plan_family_deps(root, dx_families::families())
        .map_err(|error| adopt_message(prefix, &error))?;
    notes.extend(deps.notes.iter().map(|note| format!("{prefix}{note}")));
    if let Some(edit) = deps.edit {
        dx_adopt::write_module_edit(root, &edit).map_err(|error| adopt_message(prefix, &error))?;
        let entry = format!("{prefix}{}", edit.path);
        if !applied.wrote.contains(&entry) {
            applied.wrote.push(entry);
        }
    }
    Ok(())
}

fn refresh(
    root: &Path,
    prefix: &str,
    files: &[InitFile],
    applied: &mut Applied,
) -> Result<(), String> {
    let has = |wanted: fn(&Kind) -> bool| files.iter().any(|file| wanted(&file.kind));
    let record = |applied: &mut Applied, paths: Vec<String>| {
        applied
            .wrote
            .extend(paths.into_iter().map(|path| prefix_entry(prefix, &path)));
    };
    if has(|kind| *kind == Kind::Launcher) {
        let wrote = dx_adopt::refresh_launcher_files(root)
            .map_err(|error| format!("launchers not refreshed: {error}"))?;
        record(applied, wrote);
    }
    let scaffold: Vec<ScaffoldFile> = files
        .iter()
        .filter_map(|file| match file.kind {
            Kind::Scaffold(kind) => Some(ScaffoldFile {
                path: file.path.clone(),
                content: file.content.clone(),
                kind,
            }),
            _ => None,
        })
        .collect();
    if !scaffold.is_empty() {
        let mut own = Applied::default();
        dx_adopt::refresh_scaffold(root, &scaffold, &mut own)
            .map_err(|error| format!("editor and CI files not refreshed: {error}"))?;
        record(applied, own.wrote);
    }
    if has(|kind| *kind == Kind::Override) {
        let wrote = dx_adopt::refresh_overrides(root)
            .map_err(|error| format!("overrides not refreshed: {error}"))?;
        record(applied, wrote);
    }
    if has(|kind| *kind == Kind::Preset) {
        let wrote = dx_adopt::refresh_preset(root)
            .map_err(|error| format!("preset layers not refreshed: {error}"))?;
        record(applied, wrote);
    }
    if has(|kind| *kind == Kind::Hooks) {
        let outcome =
            setup::refresh(root).map_err(|error| format!("hooks not refreshed: {error}"))?;
        record(applied, outcome.wrote);
    }
    if has(|kind| *kind == Kind::AgentsBlock)
        && agents::write(root, false)
            .map_err(|error| format!("AGENTS.md not refreshed: {error}"))?
    {
        record(applied, vec![agents::FILE.to_owned()]);
    }
    if has(|kind| *kind == Kind::ClaudeImport)
        && claude_md::refresh(root).map_err(|error| format!("CLAUDE.md not refreshed: {error}"))?
    {
        record(applied, vec![claude_md::FILE.to_owned()]);
    }
    if has(|kind| *kind == Kind::CopilotInstructions)
        && copilot_md::refresh(root)
            .map_err(|error| format!("{} not refreshed: {error}", copilot_md::FILE))?
    {
        record(applied, vec![copilot_md::FILE.to_owned()]);
    }
    Ok(())
}

/// Whether `refresh` rewrites this planned file: it exists, init may overwrite it, and it is not
/// the module or pin file, which upgrade handles itself. Keep this the one list of what refresh touches.
pub(super) fn refreshable(root: &Path, file: &InitFile) -> bool {
    root.join(&file.path).exists()
        && file.kind != Kind::Scaffold(FileKind::WriteOnce)
        && file.path != "MODULE.bazel"
        && file.path != ".dx/pins"
        && (file.path != ".bazelignore"
            || std::fs::read_to_string(root.join(".bazelignore"))
                .is_ok_and(|text| dx_adopt::bazelignore_has_bare_dx(&text)))
}

/// What a workspace lacks against what init writes.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Drift {
    pub(super) missing: Vec<String>,
    pub(super) stale: Vec<String>,
    pub(super) other: Vec<String>,
    pub(super) absent: Vec<String>,
}

/// Every planned file that is missing, stale or unreadable under `root`, without writing.
/// A missing essential file is `missing`; any other missing file is `absent`.
pub(super) fn drift(root: &Path, files: &[InitFile]) -> Drift {
    let mut drift = Drift::default();
    let stale_launchers = dx_adopt::stale_launchers(root);
    let preset = dx_adopt::preset_drift(root);
    let hooks = setup::workspace_missing(root);
    drift.other.extend(preset.other);
    for file in files {
        let path = file.path.as_str();
        if !root.join(path).exists() {
            if file.essential {
                drift.missing.push(file.path.clone());
            } else {
                drift.absent.push(file.path.clone());
            }
            continue;
        }
        match file.kind {
            Kind::Scaffold(kind) => {
                let owned = ScaffoldFile {
                    path: file.path.clone(),
                    content: file.content.clone(),
                    kind,
                };
                match dx_adopt::owned_file_drift(root, &owned) {
                    Ok(false) => {}
                    Ok(true) => drift.stale.push(file.path.clone()),
                    Err(error) => drift.other.push(error),
                }
                if path == ".bazelignore"
                    && std::fs::read_to_string(root.join(path))
                        .is_ok_and(|text| !dx_adopt::bazelignore_covers_dx(&text))
                {
                    drift.stale.push(file.path.clone());
                }
            }
            Kind::Launcher => {
                if stale_launchers.iter().any(|stale| stale == path) {
                    drift.stale.push(file.path.clone());
                }
            }
            Kind::Override => {
                if std::fs::read(root.join(path))
                    .is_ok_and(|bytes| bytes != file.content.as_bytes())
                {
                    drift.stale.push(file.path.clone());
                }
            }
            Kind::Preset => {
                if preset.stale.iter().any(|stale| stale == path) {
                    drift.stale.push(file.path.clone());
                }
            }
            Kind::Hooks => {
                if let Some((_, lacks)) = hooks.iter().find(|(hook, _)| *hook == path) {
                    drift
                        .stale
                        .push(format!("{path} (lacks hooks: {})", lacks.join("; ")));
                }
            }
            Kind::AgentsBlock => match agents::planned(root, true) {
                Ok(None) => {}
                Ok(Some(_)) => drift.stale.push(file.path.clone()),
                Err(error) => drift.other.push(format!("{path}: {error}")),
            },
            Kind::ClaudeImport => match claude_md::state(root) {
                Ok(claude_md::State::Imported | claude_md::State::Linked) => {}
                Ok(claude_md::State::Missing | claude_md::State::Absent) => drift
                    .stale
                    .push(format!("{path} (lacks @{})", agents::FILE)),
                Err(error) => drift.other.push(format!("{path}: {error}")),
            },
            Kind::CopilotInstructions => match copilot_md::imported(root) {
                Ok(true) => {}
                Ok(false) => drift.stale.push(format!("{path} (lacks {})", agents::FILE)),
                Err(error) => drift.other.push(format!("{path}: {error}")),
            },
        }
    }
    drift
}

#[cfg(test)]
mod tests {
    use super::plan;
    use crate::adopt::test_support::{drift_of, invocation, run};
    use dx_plan::args::Agent;

    #[test]
    fn init_refuses_when_module_bazel_is_ahead_of_the_binary() {
        let scratch = dx_test_scratch::scratch("dx-init-ahead-");
        let root = scratch.path();
        std::fs::write(
            root.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("MODULE.bazel pins rules_dx 9.9.9"), "{err}");
        assert!(!root.join(".dx/pins").exists());
    }

    #[test]
    fn everything_init_writes_is_owned_and_present() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-");
        let root = scratch.path();
        let files = plan(root, "my_project", true, &Agent::ALL).expect("plan");
        let (code, out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        for path in out.lines().filter_map(|line| line.strip_prefix("wrote ")) {
            assert!(files.iter().any(|file| file.path == path), "{path}");
        }
        for file in &files {
            let accounted = root.join(&file.path).exists()
                || out.contains(&file.path)
                || err.contains(&file.path);
            assert!(accounted, "{}", file.path);
        }
        scratch.close().expect("cleanup");
    }

    const INIT_DOC: &str = include_str!("../../../../docs/cli/commands/init.md");

    /// The `(paths, label)` of each row in the "Fixed file set" table.
    fn doc_rows() -> Vec<(Vec<String>, String)> {
        let table = INIT_DOC
            .split("## Fixed file set")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .expect("fixed file set section");
        table
            .lines()
            .filter(|line| line.starts_with("| `"))
            .map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                let paths = cells[1]
                    .split('`')
                    .skip(1)
                    .step_by(2)
                    .map(str::to_owned)
                    .collect();
                (paths, cells[2].to_owned())
            })
            .collect()
    }

    #[test]
    fn init_doc_has_one_row_per_planned_file_with_the_ownership_label() {
        let scratch = dx_test_scratch::scratch("dx-init-doc-rows-");
        let root = scratch.path();
        std::fs::write(root.join("app.py"), "print(1)\n").expect("source");
        let rows = doc_rows();
        let files = plan(root, "my_project", true, &Agent::ALL).expect("plan");
        for file in &files {
            let matching: Vec<&String> = rows
                .iter()
                .filter(|(paths, _)| {
                    paths.iter().any(|path| {
                        *path == file.path
                            || (path.ends_with('/') && file.path.starts_with(path.as_str()))
                    })
                })
                .map(|(_, label)| label)
                .collect();
            assert_eq!(matching.len(), 1, "{}: rows {matching:?}", file.path);
            assert_eq!(
                matching[0].as_str(),
                file.ownership().label(),
                "{}",
                file.path
            );
        }
        for (paths, _) in &rows {
            for path in paths {
                assert!(
                    files.iter().any(|file| file.path == *path
                        || (path.ends_with('/') && file.path.starts_with(path.as_str()))),
                    "row {path} is not planned"
                );
            }
        }
    }

    #[test]
    fn init_doc_describes_every_ownership_label() {
        for ownership in dx_adopt::Ownership::ALL {
            let entry = format!("- `{}`: ", ownership.label());
            assert!(INIT_DOC.contains(&entry), "{}", ownership.label());
        }
    }

    #[test]
    fn every_planned_file_has_content() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-content-");
        for file in plan(scratch.path(), "my_project", true, &Agent::ALL).expect("plan") {
            assert!(!file.content.is_empty(), "{}", file.path);
        }
    }

    #[test]
    fn each_path_is_listed_once() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-once-");
        let files = plan(scratch.path(), "my_project", true, &Agent::ALL).expect("plan");
        let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        paths.sort_unstable();
        let count = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), count);
        assert!(
            files
                .iter()
                .filter(|file| file.path == ".gitignore")
                .count()
                <= 1
        );
    }

    #[test]
    fn apply_writes_the_owned_list_and_check_and_status_accept_it() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-equal-");
        let root = scratch.path();
        let mut owned_paths: Vec<String> = plan(root, "my_project", true, &Agent::ALL)
            .expect("plan")
            .into_iter()
            .map(|file| file.path)
            .collect();
        let (code, out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        let mut wrote: Vec<String> = out
            .lines()
            .filter_map(|line| line.strip_prefix("wrote "))
            .map(str::to_owned)
            .collect();
        wrote.sort();
        owned_paths.sort();
        assert_eq!(wrote, owned_paths);
        assert_eq!(drift_of(root), super::Drift::default());
        let (code, out, err) = run(&invocation(&["init"]), root);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("init current"), "{out}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn apply_wires_the_root_lock_files_once() {
        let scratch = dx_test_scratch::scratch("dx-init-lock-hubs-");
        let root = scratch.path();
        std::fs::write(root.join("MODULE.bazel"), "module(name = \"app\")\n").expect("write");
        std::fs::write(root.join("pnpm-lock.yaml"), "").expect("write");
        std::fs::write(root.join("package.json"), "{}").expect("write");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        let module = std::fs::read_to_string(root.join("MODULE.bazel")).expect("read");
        assert!(
            module.contains("pnpm_lock = \"//:pnpm-lock.yaml\""),
            "{module}"
        );
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        let again = std::fs::read_to_string(root.join("MODULE.bazel")).expect("read");
        assert_eq!(again, module);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn apply_adds_no_family_module_for_a_python_workspace() {
        let scratch = dx_test_scratch::scratch("dx-init-family-deps-");
        let root = scratch.path();
        let module =
            "module(name = \"app\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";
        std::fs::write(root.join("MODULE.bazel"), module).expect("write");
        std::fs::write(root.join("app.py"), "").expect("write");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        let after = std::fs::read_to_string(root.join("MODULE.bazel")).expect("read");
        assert!(!after.contains("rules_dx_"), "{after}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn drift_in_an_empty_dir_is_owned() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-drift-");
        let root = scratch.path();
        let files = plan(root, "my_project", true, &Agent::ALL).expect("plan");
        for path in drift_of(root).missing {
            assert!(files.iter().any(|file| file.path == path), "{path}");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_removed_planned_file_is_missing_only_when_essential() {
        let scratch = dx_test_scratch::scratch("dx-init-planned-removed-");
        let root = scratch.path();
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        let files = plan(root, "my_project", true, &Agent::ALL).expect("plan");
        for file in &files {
            let full = root.join(&file.path);
            let Ok(kept) = std::fs::read(&full) else {
                continue;
            };
            let mode = std::fs::metadata(&full).expect("metadata").permissions();
            std::fs::remove_file(&full).expect("remove");
            let drift = super::drift(root, &files);
            assert_eq!(
                drift.missing.contains(&file.path),
                file.essential,
                "{}",
                file.path
            );
            assert_eq!(
                drift.absent.contains(&file.path),
                !file.essential,
                "{}",
                file.path
            );
            if file.path != "MODULE.bazel" {
                let (code, _out, _err) = run(
                    &invocation(&["init", "--agents=claude,codex,gemini,copilot"]),
                    root,
                );
                assert_eq!(code == 0, !file.essential, "{}", file.path);
            }
            std::fs::write(&full, kept).expect("restore");
            std::fs::set_permissions(&full, mode).expect("mode");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn removing_an_owned_file_after_init_is_reported_missing() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-removed-");
        let root = scratch.path();
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        let mut paths = vec![
            dx_adopt::LAUNCHER_SH_REL.to_owned(),
            dx_adopt::LAUNCHER_CMD_REL.to_owned(),
        ];
        paths.push(dx_adopt::preset::GITIGNORE.to_owned());
        paths.extend(
            dx_adopt::render_overrides(&Default::default())
                .into_iter()
                .map(|(p, _)| p),
        );
        let files = plan(root, "my_project", true, &Agent::ALL).expect("plan");
        for path in paths {
            assert!(files.iter().any(|file| file.path == path), "{path}");
            let full = root.join(&path);
            let kept = std::fs::read(&full).expect("read");
            std::fs::remove_file(&full).expect("remove");
            assert!(drift_of(root).missing.contains(&path), "{path}");
            std::fs::write(&full, kept).expect("restore");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn drift_names_each_owned_gap() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-gaps-");
        let root = scratch.path();
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        assert_eq!(drift_of(root), super::Drift::default());
        std::fs::write(root.join(".bazelignore"), "node_modules\n").expect("ignore");
        std::fs::write(
            root.join("tools/dx"),
            "#!/bin/sh\nset -eu\nDX_PINS=.dx/pins\n",
        )
        .expect("dx");
        std::fs::write(root.join(".dx/pins"), "4.5.6\n").expect("pin");
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("agents");
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("claude");
        let (path, _) = dx_adopt::render_overrides(&Default::default()).remove(0);
        std::fs::write(root.join(&path), "changed\n").expect("override");
        let stale = drift_of(root).stale.join("\n");
        for want in [
            ".bazelignore",
            "tools/dx",
            "AGENTS.md",
            "lacks @AGENTS.md",
            path.as_str(),
        ] {
            assert!(stale.contains(want), "{want}: {stale}");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn old_dx_keys_in_vscode_settings_are_stale() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-vscode-");
        let root = scratch.path();
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        let path = root.join(".vscode/settings.json");
        let text = std::fs::read_to_string(&path).expect("read");
        let (key, _) = text.split_once(':').expect("key");
        std::fs::write(&path, format!("{key}: 1,\n\"x\": 1}}\n")).expect("write");
        assert!(drift_of(root)
            .stale
            .contains(&".vscode/settings.json".to_owned()));
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        assert_eq!(drift_of(root), super::Drift::default());
        scratch.close().expect("cleanup");
    }
}
