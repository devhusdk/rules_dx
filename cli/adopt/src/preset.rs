//! The Bazel preset in a consumer workspace: the root `.bazelrc` imports of the
//! `.dx/bazelrc/*.bazelrc` layers `dx generate` writes, and the `.gitignore` lines.
//!
//! One reader ([`inspect_preset`]) feeds init, status and upgrade, so they agree.

use std::path::{Path, PathBuf};

use crate::Ownership;
use crate::build_output::IGNORE_LINES;
use crate::error::AdoptError;
use crate::preset_fragment::{
    CONSUMER_DIR, ImportState, Layer, layer_import_state, owned_collisions_in_content,
    prepend_imports, render_all,
};
use crate::scaffold::{Applied, scaffold_dest_within_root, with_header};

/// The root `.bazelrc` file.
pub const BAZELRC: &str = ".bazelrc";

/// The root `.gitignore` file.
pub const GITIGNORE: &str = ".gitignore";

/// A workspace's preset: the root `.bazelrc` imports and the layer files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresetState {
    /// The root `.bazelrc` text, empty when missing.
    pub rc_text: String,
    pub rc_exists: bool,
    pub rc_symlink: bool,
    /// The `.gitignore` text with each `IGNORE_LINES` entry it lacks added; the whole set when the
    /// file is missing. None when nothing is missing.
    pub ignore_text: Option<String>,
    /// Owned preset lines the root `.bazelrc` repeats.
    pub collisions: Vec<String>,
    pub layers: [(Layer, ImportState); 3],
}

impl PresetState {
    /// True when no line, active or commented, names a `.dx/bazelrc` layer.
    pub fn unwired(&self) -> bool {
        self.layers
            .iter()
            .all(|(_, import)| *import == ImportState::Absent)
    }

    /// Whether apply would add the import block.
    pub fn will_import(&self) -> bool {
        self.unwired() && !self.rc_symlink && self.collisions.is_empty()
    }

    /// Some layers are named in the `.bazelrc` and some are not.
    pub fn partial(&self) -> bool {
        !self.unwired()
            && self
                .layers
                .iter()
                .any(|(_, import)| *import == ImportState::Absent)
    }

    /// Advisories for what apply leaves alone.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.unwired() {
            if self.rc_symlink {
                notes.push(format!(
                    "{BAZELRC} is a symlink; add these imports yourself: {}",
                    self.import_lines().join("; ")
                ));
            } else if !self.collisions.is_empty() {
                notes.push(
                    dx_preset::PresetError::OwnedCollision {
                        lines: self.collisions.join(", "),
                    }
                    .to_string()
                        + "; preset imports not added",
                );
            }
        } else if self.partial() {
            let absent: Vec<_> = self
                .layers
                .iter()
                .filter(|(_, import)| *import == ImportState::Absent)
                .map(|(layer, _)| layer.file_name())
                .collect();
            notes.push(format!(
                "{BAZELRC} does not import {}; add the import or comment it out to opt out",
                absent.join(", ")
            ));
        }
        notes
    }

    fn import_lines(&self) -> Vec<String> {
        Layer::ALL
            .iter()
            .map(|layer| dx_preset::import_line(CONSUMER_DIR, *layer))
            .collect()
    }
}

/// The workspace-relative path of one layer file.
pub fn layer_rel(layer: Layer) -> String {
    format!("{CONSUMER_DIR}/{}", layer.file_name())
}

/// Every workspace-relative path the preset owns.
pub fn preset_paths() -> Vec<String> {
    let mut paths = vec![BAZELRC.to_owned(), GITIGNORE.to_owned()];
    paths.extend(Layer::ALL.iter().map(|layer| layer_rel(*layer)));
    paths
}

/// Every file the preset owns with the content a fresh workspace gets.
pub fn preset_files() -> Vec<(String, String)> {
    let imports: Vec<String> = Layer::ALL
        .iter()
        .map(|layer| dx_preset::import_line(CONSUMER_DIR, *layer))
        .collect();
    vec![
        (
            BAZELRC.to_owned(),
            with_header(
                Ownership::Lines,
                BAZELRC,
                &format!("{}\n", imports.join("\n")),
            ),
        ),
        (
            GITIGNORE.to_owned(),
            with_header(
                Ownership::Lines,
                GITIGNORE,
                &format!("{}\n", IGNORE_LINES.join("\n")),
            ),
        ),
    ]
}

fn read_text(path: &Path, rel: &str) -> Result<Option<String>, AdoptError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AdoptError::ReadFile {
            path: rel.to_owned(),
            detail: e.to_string(),
        }),
    }
}

/// Reads the preset state of `root` once.
pub fn inspect_preset(root: &Path) -> Result<PresetState, AdoptError> {
    let rc_path = root.join(BAZELRC);
    let rc_symlink = std::fs::symlink_metadata(&rc_path).is_ok_and(|m| m.file_type().is_symlink());
    let text = read_text(&rc_path, BAZELRC)?;
    let rc_exists = text.is_some();
    let rc_text = text.unwrap_or_default();
    let ignore_text = ignore_with_missing(read_text(&root.join(GITIGNORE), GITIGNORE)?);
    let repo = dx_module_bazel::rules_dx_repo(root);
    let layers = layer_import_state(&rc_text, CONSUMER_DIR);
    Ok(PresetState {
        collisions: owned_collisions_in_content(&rc_text, &render_all(&repo)),
        rc_text,
        rc_exists,
        rc_symlink,
        ignore_text,
        layers,
    })
}

/// Whether `lines` already ignore `entry`: a line equal to it, or a listed ancestor directory.
/// A leading or trailing slash does not count.
fn ignores(lines: &[&str], entry: &str) -> bool {
    let entry = entry.trim_matches('/');
    lines.iter().any(|line| {
        let line = line.trim().trim_matches('/');
        !line.is_empty()
            && (entry == line
                || entry
                    .strip_prefix(line)
                    .is_some_and(|rest| rest.starts_with('/')))
    })
}

fn ignore_with_missing(text: Option<String>) -> Option<String> {
    let new_file = text.is_none();
    let mut text = text.unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let missing: Vec<&str> = IGNORE_LINES
        .into_iter()
        .filter(|entry| !ignores(&lines, entry))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if new_file {
        text = with_header(Ownership::Lines, GITIGNORE, "");
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(eol);
    }
    for entry in missing {
        text.push_str(entry);
        text.push_str(eol);
    }
    Some(text)
}

/// Whether git lists files under `.dx/bazelrc`; false when git is missing or this is no repository.
fn layers_tracked(root: &Path) -> bool {
    dx_process::capture::run_checked(
        dx_process::capture::git(root).args(["ls-files", "--", CONSUMER_DIR]),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .is_ok_and(|out| !out.stdout.is_empty())
}

fn tracked_note() -> String {
    format!(
        "{CONSUMER_DIR} is committed; run `git rm --cached -r {CONSUMER_DIR}` to stop tracking it"
    )
}

fn write(root: &Path, rel: &str, content: &str) -> Result<PathBuf, AdoptError> {
    let dest = scaffold_dest_within_root(root, rel)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
            parent: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    dx_atomic_fs::write_atomic(&dest, content.as_bytes()).map_err(|e| AdoptError::WriteFile {
        path: dest.display().to_string(),
        detail: e.to_string(),
    })?;
    Ok(dest)
}

/// Fails before any write when a preset destination escapes the workspace.
pub fn preflight_preset(root: &Path) -> Result<(), AdoptError> {
    inspect_preset(root)?;
    scaffold_dest_within_root(root, BAZELRC)?;
    scaffold_dest_within_root(root, GITIGNORE)?;
    Ok(())
}

/// Adds the import block when that is safe and the missing `.gitignore` lines.
///
/// Paths written land in `applied`; advisories land in `notes`.
pub fn write_preset(
    root: &Path,
    applied: &mut Applied,
    notes: &mut Vec<String>,
) -> Result<(), AdoptError> {
    let state = inspect_preset(root)?;
    notes.extend(state.notes());
    if state.will_import() {
        let pin = std::fs::read_to_string(root.join(".bazelversion")).unwrap_or_default();
        notes.extend(crate::status::old_pin_note(&pin));
        let mut text = prepend_imports(&state.rc_text, CONSUMER_DIR, &Layer::ALL);
        if !state.rc_exists {
            text = with_header(Ownership::Lines, BAZELRC, &text);
        }
        write(root, BAZELRC, &text)?;
        applied.wrote.push(BAZELRC.to_owned());
    }
    if let Some(text) = &state.ignore_text {
        write(root, GITIGNORE, text)?;
        applied.wrote.push(GITIGNORE.to_owned());
    }
    if layers_tracked(root) {
        notes.push(tracked_note());
    }
    Ok(())
}

/// What a workspace lacks against what [`write_preset`] writes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PresetDrift {
    pub missing: Vec<String>,
    pub stale: Vec<String>,
    pub other: Vec<String>,
}

/// The preset drift under `root`, without writing.
pub fn preset_drift(root: &Path) -> PresetDrift {
    let mut drift = PresetDrift::default();
    let state = match inspect_preset(root) {
        Ok(state) => state,
        Err(error) => {
            drift.other.push(error.to_string());
            return drift;
        }
    };
    drift.other.extend(state.notes());
    if state.will_import() {
        if state.rc_exists {
            drift.stale.push(BAZELRC.to_owned());
        } else {
            drift.missing.push(BAZELRC.to_owned());
        }
    }
    if state.ignore_text.is_some() {
        drift.stale.push(GITIGNORE.to_owned());
    }
    if layers_tracked(root) {
        drift.other.push(tracked_note());
    }
    drift
}

#[cfg(test)]
mod tests {
    #[test]
    fn preset_files_are_the_rc_and_the_ignore_file() {
        let files: Vec<String> = super::preset_files()
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert_eq!(files, [BAZELRC, GITIGNORE]);
        assert!(
            super::preset_files()
                .iter()
                .all(|(_, content)| !content.is_empty())
        );
    }

    use super::*;

    fn apply(root: &Path) -> (Vec<String>, Vec<String>) {
        let (mut applied, mut notes) = (Applied::default(), Vec::new());
        write_preset(root, &mut applied, &mut notes).expect("write");
        (applied.wrote, notes)
    }

    #[test]
    fn empty_dir_gets_layers_and_imports_then_nothing() {
        let scratch = dx_test_scratch::scratch("dx-preset-empty-");
        let root = scratch.path();
        let (wrote, notes) = apply(root);
        assert_eq!(wrote, [BAZELRC, GITIGNORE]);
        assert!(notes.is_empty());
        assert_eq!(preset_drift(root), PresetDrift::default());
        let (wrote, notes) = apply(root);
        assert!(wrote.is_empty() && notes.is_empty(), "{wrote:?} {notes:?}");
    }

    #[test]
    fn existing_bazelrc_keeps_its_bytes_below_the_block() {
        let scratch = dx_test_scratch::scratch("dx-preset-rc-");
        let root = scratch.path();
        std::fs::write(root.join(".bazelrc"), "build --jobs=2\r\n").expect("rc");
        apply(root);
        let text = std::fs::read_to_string(root.join(".bazelrc")).expect("read");
        assert!(
            text.ends_with("\r\n\r\nbuild --jobs=2\r\n\r\ntry-import %workspace%/user.bazelrc\r\n"),
            "{text:?}"
        );
        assert!(text.starts_with(dx_preset::IMPORT_COMMENT));
    }

    #[test]
    fn telemetry_opt_out_still_gets_imports() {
        let scratch = dx_test_scratch::scratch("dx-preset-telemetry-");
        let root = scratch.path();
        std::fs::write(root.join(".bazelrc"), "common --repo_env=DO_NOT_TRACK=1\n").expect("rc");
        let (wrote, _) = apply(root);
        assert!(!wrote.is_empty());
        let text = std::fs::read_to_string(root.join(".bazelrc")).expect("read");
        assert!(text.starts_with(dx_preset::IMPORT_COMMENT), "{text:?}");
    }

    #[test]
    fn collision_and_symlinked_rc_add_no_import() {
        let scratch = dx_test_scratch::scratch("dx-preset-collide-");
        let root = scratch.path();
        std::fs::write(root.join(".bazelrc"), "build --verbose_failures\n").expect("rc");
        let (wrote, notes) = apply(root);
        assert_eq!(wrote, [GITIGNORE.to_owned()]);
        assert!(notes[0].contains("duplicates preset lines"), "{notes:?}");
        assert_eq!(
            std::fs::read_to_string(root.join(".bazelrc")).expect("read"),
            "build --verbose_failures\n"
        );
        #[cfg(unix)]
        {
            let other = tempdir_sibling(root);
            std::fs::write(&other, "").expect("target");
            std::fs::remove_file(root.join(".bazelrc")).expect("rm");
            std::os::unix::fs::symlink(&other, root.join(".bazelrc")).expect("link");
            let (wrote, notes) = apply(root);
            assert!(wrote.is_empty());
            assert!(notes[0].contains("symlink"), "{notes:?}");
        }
    }

    #[cfg(unix)]
    fn tempdir_sibling(root: &Path) -> PathBuf {
        root.join("real.bazelrc")
    }

    #[test]
    fn commented_import_opts_a_layer_out_and_unlisted_one_is_noted() {
        let scratch = dx_test_scratch::scratch("dx-preset-optout-");
        let root = scratch.path();
        apply(root);
        let resources = dx_preset::import_line(CONSUMER_DIR, Layer::Resources);
        let rc = std::fs::read_to_string(root.join(".bazelrc")).expect("rc");
        std::fs::write(
            root.join(".bazelrc"),
            rc.replace(&resources, &format!("# {resources}")),
        )
        .expect("comment");
        assert_eq!(preset_drift(root), PresetDrift::default());
        assert!(apply(root).0.is_empty());
        std::fs::write(
            root.join(".bazelrc"),
            rc.replace(&format!("{resources}\n"), ""),
        )
        .expect("delete");
        let drift = preset_drift(root);
        assert!(drift.other[0].contains("resources.bazelrc"), "{drift:?}");
        assert!(apply(root).0.is_empty());
    }

    #[test]
    fn wiring_appends_the_missing_ignore_lines() {
        let scratch = dx_test_scratch::scratch("dx-preset-ignore-");
        let root = scratch.path();
        std::fs::write(root.join(".bazelrc"), "build --jobs=2\n").expect("rc");
        std::fs::write(root.join(".gitignore"), "target/\nmine.log").expect("ignore");
        assert_eq!(preset_drift(root).stale, vec![BAZELRC, GITIGNORE]);
        let (wrote, _) = apply(root);
        assert_eq!(wrote, [BAZELRC, GITIGNORE]);
        let ignore = std::fs::read_to_string(root.join(".gitignore")).expect("ignore");
        let added: String = IGNORE_LINES[1..]
            .iter()
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(ignore, format!("target/\nmine.log\n{added}"));
        assert_eq!(preset_drift(root), PresetDrift::default());
        assert!(apply(root).0.is_empty());
    }

    #[test]
    fn target_only_gitignore_gains_every_other_line() {
        let text = ignore_with_missing(Some("target/\n".to_owned())).expect("missing");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "target/");
        assert_eq!(lines[1..], IGNORE_LINES[1..]);
        assert_eq!(ignore_with_missing(Some(text)), None);
    }

    #[test]
    fn an_equal_or_ancestor_line_ignores_the_entry() {
        for line in [".dx/", "/.dx", "/.dx/bazelrc", ".dx/bazelrc/", "bazel-*"] {
            let entry = if line == "bazel-*" {
                "/bazel-*"
            } else {
                "/.dx/bazelrc/"
            };
            assert!(ignores(&[line], entry), "{line}");
        }
        for line in ["dx/", ".dx/bazel", "!.dx/", "# /.dx/bazelrc/"] {
            assert!(!ignores(&[line], "/.dx/bazelrc/"), "{line}");
        }
        assert_eq!(
            ignore_with_missing(None).expect("all").lines().count(),
            IGNORE_LINES.len() + 1
        );
    }

    fn git(root: &Path, sandbox: &Path, args: &[&str]) {
        let mut cmd = dx_process::capture::git(root);
        cmd.args(args);
        dx_test_scratch::isolate_git(&mut cmd, sandbox);
        dx_process::capture::run_checked(&mut cmd, dx_process::capture::DEFAULT_TIMEOUT)
            .unwrap_or_else(|error| panic!("{args:?}: {error}"));
    }

    #[test]
    fn tracked_layers_get_the_git_rm_note() {
        let scratch = dx_test_scratch::scratch("dx-preset-tracked-");
        let root = scratch.path().join("repo");
        std::fs::create_dir_all(&root).expect("dir");
        git(&root, scratch.path(), &["init", "-q"]);
        apply(&root);
        write(&root, &layer_rel(Layer::Resources), "build --jobs=2\n").expect("layer");
        git(&root, scratch.path(), &["add", "-f", CONSUMER_DIR]);
        assert!(preset_drift(&root).other.contains(&tracked_note()));
        assert!(apply(&root).1.contains(&tracked_note()));
    }
}
