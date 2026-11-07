//! Tool config profiles and the notes `dx init` prints about root tool configs.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use dx_generate::tools_gen::{TOOL_CONFIGS, ToolConfig};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Profile {
    #[default]
    Standard,
    Strict,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }
}

/// How a tool's root file relates to the profile files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// The root file holds an extends line naming the chosen base.
    Inherit,
    /// The root file is a copy of the chosen base.
    Copy,
}

struct Entry {
    tool: &'static str,
    family: &'static str,
    /// Extensions that mean the tool applies, for tools no family owns.
    extensions: &'static [&'static str],
    ext: &'static str,
    root: &'static str,
    /// The root file of an inherit tool, with `{base}` for the chosen base path.
    root_template: &'static str,
    mode: Mode,
    /// Root files that mean the repo already configures the tool; a trailing `*` matches a prefix.
    configured: &'static [&'static str],
    /// Table in `pyproject.toml` that also means the tool is configured.
    pyproject_table: Option<&'static str>,
    standard: &'static str,
    strict: &'static str,
}

static TABLE: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    TOOL_CONFIGS
        .iter()
        .filter(|config| !config.ext.is_empty())
        .map(Entry::from_config)
        .collect()
});

impl Entry {
    fn from_config(config: &'static ToolConfig) -> Self {
        let present = |text: &'static str| (!text.is_empty()).then_some(text);
        Self {
            tool: if config.tier_dir.is_empty() {
                config.id
            } else {
                config.tier_dir
            },
            family: config.family,
            extensions: config.extensions,
            ext: config.ext,
            root: config.files.first().copied().unwrap_or_default(),
            root_template: config.extends_syntax,
            mode: if config.extends_syntax.is_empty() {
                Mode::Copy
            } else {
                Mode::Inherit
            },
            configured: config.files,
            pyproject_table: present(config.pyproject_table),
            standard: config.standard,
            strict: config.strict,
        }
    }

    fn base_path(&self, profile: Profile) -> String {
        format!(".dx/config/{}/{}.{}", self.tool, profile.name(), self.ext)
    }

    fn template(&self, profile: Profile) -> &'static str {
        match profile {
            Profile::Standard => self.standard,
            Profile::Strict => self.strict,
        }
    }
}

fn already_configured(root: &Path, entry: &Entry) -> bool {
    let named = |name: &str| match name.strip_suffix('*') {
        Some(prefix) => std::fs::read_dir(root).is_ok_and(|dir| {
            dir.flatten()
                .any(|file| file.file_name().to_string_lossy().starts_with(prefix))
        }),
        None => root.join(name).is_file(),
    };
    if entry.configured.iter().any(|name| named(name)) {
        return true;
    }
    let Some(table) = entry.pyproject_table else {
        return false;
    };
    std::fs::read_to_string(root.join("pyproject.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|parsed| {
            parsed
                .get("tool")
                .and_then(|tool| tool.get(table))
                .is_some()
        })
}

fn root_content(entry: &Entry, profile: Profile) -> String {
    match entry.mode {
        Mode::Inherit => entry
            .root_template
            .replace("{base}", &entry.base_path(profile)),
        Mode::Copy => entry.template(profile).to_owned(),
    }
}

fn detected(entry: &Entry, extensions: &BTreeSet<String>) -> bool {
    if entry.extensions.iter().any(|ext| extensions.contains(*ext)) {
        return true;
    }
    dx_families::families()
        .iter()
        .filter(|family| family.name == entry.family)
        .flat_map(|family| &family.file_extensions)
        .any(|ext| extensions.contains(&ext.to_ascii_lowercase()))
}

fn entries<'a>(extensions: &'a BTreeSet<String>) -> impl Iterator<Item = &'static Entry> + 'a {
    TABLE.iter().filter(|entry| detected(entry, extensions))
}

/// The profile the root files of `root` already pick; `Standard` when they say nothing.
pub fn infer_profile(root: &Path, extensions: &BTreeSet<String>) -> Profile {
    let picks_strict = entries(extensions).any(|entry| {
        let text = std::fs::read_to_string(root.join(entry.root)).unwrap_or_default();
        match entry.mode {
            Mode::Inherit => text.contains(&entry.base_path(Profile::Strict)),
            Mode::Copy => text == entry.template(Profile::Strict),
        }
    });
    if picks_strict {
        Profile::Strict
    } else {
        Profile::Standard
    }
}

fn is_ours(root: &Path, entry: &Entry) -> bool {
    std::fs::read_to_string(root.join(entry.root))
        .is_ok_and(|text| text.contains(&format!(".dx/config/{}/", entry.tool)))
}

/// One line for each tool whose own config was kept, so the root file is not planned.
pub fn kept_config_notes(root: &Path) -> Vec<String> {
    let extensions = crate::families::file_extensions(root);
    entries(&extensions)
        .filter(|entry| already_configured(root, entry) && !is_ours(root, entry))
        .map(|entry| {
            format!(
                "{}: existing config kept; bases are in .dx/config/{}/",
                entry.root, entry.tool
            )
        })
        .collect()
}

/// One line for each root file that exists and does not pick `profile`.
pub fn profile_notes(root: &Path, profile: Profile) -> Vec<String> {
    let extensions = crate::families::file_extensions(root);
    entries(&extensions)
        .filter_map(|entry| {
            if !is_ours(root, entry) {
                return None;
            }
            let existing = std::fs::read_to_string(root.join(entry.root)).ok()?;
            let wanted = root_content(entry, profile);
            if existing == wanted {
                return None;
            }
            let change = match entry.mode {
                Mode::Inherit => format!("point it at {}", entry.base_path(profile)),
                Mode::Copy => format!("copy {}", entry.base_path(profile)),
            };
            Some(format!(
                "{} is left as is for --profile {}: {change}",
                entry.root,
                profile.name()
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_both_templates_that_parse() {
        for entry in TABLE.iter() {
            for profile in [Profile::Standard, Profile::Strict] {
                let text = entry.template(profile);
                assert!(!text.is_empty(), "{} {}", entry.tool, profile.name());
                match entry.ext {
                    "toml" => {
                        text.parse::<toml::Table>().expect("toml base");
                    }
                    "json" => {
                        serde_json::from_str::<serde_json::Value>(text).expect("json base");
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn copy_mode_copies_the_chosen_base_to_the_root_path() {
        let entry = Entry {
            tool: "demo",
            family: "python",
            extensions: &[],
            ext: "toml",
            root: "demo.toml",
            root_template: "",
            mode: Mode::Copy,
            configured: &[],
            pyproject_table: None,
            standard: "a = 1\n",
            strict: "a = 2\n",
        };
        assert_eq!(root_content(&entry, Profile::Strict), "a = 2\n");
    }

    #[test]
    fn a_kept_pyproject_config_is_noted() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let root = scratch.path();
        std::fs::write(root.join("pyproject.toml"), "[tool.ruff]\n").expect("pyproject");
        std::fs::write(root.join("a.py"), "").expect("py");
        assert_eq!(
            kept_config_notes(root),
            ["ruff.toml: existing config kept; bases are in .dx/config/ruff/"]
        );
    }
}
