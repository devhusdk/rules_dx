//! Moves a workspace off the old `.dx/config/<tool>/{standard,strict}.*` layout.
//!
//! Old `dx init` wrote both tier files and root files that point at one of them. dx now renders
//! one `base.<ext>` per tool and reads the tier from `dx.toml`, so the root references are
//! rewritten and a strict workspace gets `tier = "strict"`.

use std::collections::BTreeSet;
use std::path::Path;

use dx_generate::tools_gen::TOOL_CONFIGS;

use crate::AdoptError;

const DX_TOML: &str = "dx.toml";
const CORRECTNESS_LAYER: &str = ".dx/bazelrc/correctness.bazelrc";

/// Rewrites root files that reference the old layout and records a strict tier in `dx.toml`.
/// A reference is rewritten only when dx renders the new base for this repo; the sync keeps
/// the old file otherwise. Returns one line per change; a second run returns nothing.
pub fn migrate(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut lines = Vec::new();
    let mut old_layout = false;
    let mut strict = false;
    let mut rendered: Option<BTreeSet<String>> = None;
    let mut rewrites = Vec::new();
    for config in TOOL_CONFIGS.iter().filter(|config| !config.ext.is_empty()) {
        let dir = dx_generate::tier_dir(config);
        let old = |tier: &str| format!(".dx/config/{dir}/{tier}.{}", config.ext);
        old_layout |= ["standard", "strict"]
            .iter()
            .any(|tier| root.join(old(tier)).is_file());
        for name in config.files.iter().filter(|name| !name.ends_with('*')) {
            let Some(text) = read_optional(root, name)? else {
                continue;
            };
            if config.extends_syntax.is_empty()
                && !config.strict.is_empty()
                && config.strict != config.standard
                && text == config.strict
            {
                strict = true;
            }
            let mut new_text = text.clone();
            for tier in ["standard", "strict"] {
                let old = old(tier);
                if !text.contains(&old) {
                    continue;
                }
                old_layout = true;
                strict |= tier == "strict";
                let Some(new) = dx_generate::base_path(config) else {
                    continue;
                };
                let rendered = match &mut rendered {
                    Some(rendered) => rendered,
                    None => rendered.insert(planned(root)?),
                };
                if rendered.contains(&new) {
                    new_text = new_text.replace(&old, &new);
                    lines.push(format!("dx: migrated {name}: {old} -> {new}"));
                }
            }
            if new_text != text {
                rewrites.push((*name, new_text));
            }
        }
    }
    let layer = read_optional(root, CORRECTNESS_LAYER)?;
    strict |= layer
        .as_ref()
        .is_some_and(|text| text.contains("warnings_as_errors=true"));
    if old_layout && strict && set_strict_tier(root)? {
        lines.push(format!("dx: migrated {DX_TOML}: tier = \"strict\""));
    }
    if old_layout && !strict && layer.is_some_and(|text| !text.contains("warnings_as_errors")) {
        lines.push(format!(
            "dx: note: the old correctness layer made warnings errors; the tier is now \
             standard, set tier = \"strict\" in {DX_TOML} to keep errors"
        ));
    }
    for (name, text) in rewrites {
        write(root, name, &text)?;
    }
    Ok(lines)
}

/// The paths a sync of `root` would write.
fn planned(root: &Path) -> Result<BTreeSet<String>, AdoptError> {
    dx_generate::plan(root, &mut crate::detected_families)
        .map(|files| files.into_keys().collect())
        .map_err(|error| AdoptError::LegacyLayout {
            detail: error.to_string(),
        })
}

/// Adds `tier = "strict"` unless `dx.toml` already sets a tier. Fails when `dx.toml` does not
/// parse, so the old files stay until the tier can be written.
fn set_strict_tier(root: &Path) -> Result<bool, AdoptError> {
    let line = "tier = \"strict\"\n";
    let text = match read_optional(root, DX_TOML)? {
        None => format!("schema_version = 1\n{line}"),
        Some(text) => {
            let table = text
                .parse::<toml::Table>()
                .map_err(|error| AdoptError::LegacyLayout {
                    detail: format!("{DX_TOML}: {error}"),
                })?;
            if table.contains_key("tier") {
                return Ok(false);
            }
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            let at = lines
                .iter()
                .position(|line| line.trim_start().starts_with("schema_version"))
                .map(|index| index + 1)
                .or_else(|| {
                    lines
                        .iter()
                        .position(|line| line.trim_start().starts_with('['))
                })
                .unwrap_or(lines.len());
            let mut out: String = lines[..at].concat();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(line);
            out.push_str(&lines[at..].concat());
            out
        }
    };
    write(root, DX_TOML, &text)?;
    Ok(true)
}

fn read_optional(root: &Path, rel: &str) -> Result<Option<String>, AdoptError> {
    let path = root.join(rel);
    if !path.is_file() {
        return Ok(None);
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(error) => Err(AdoptError::ReadFile {
            path: rel.to_owned(),
            detail: error.to_string(),
        }),
    }
}

fn write(root: &Path, rel: &str, text: &str) -> Result<(), AdoptError> {
    dx_atomic_fs::write_atomic(&root.join(rel), text.as_bytes()).map_err(|error| {
        AdoptError::WriteFile {
            path: rel.to_owned(),
            detail: error.to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::write as put;

    const ESLINT: &str = "eslint.config.mjs";
    const STYLELINT: &str = ".stylelintrc.json";

    fn read(root: &Path, rel: &str) -> String {
        std::fs::read_to_string(root.join(rel)).expect(rel)
    }

    /// An old-layout repo with ruff, eslint, stylelint and tsconfig roots, and the given sources.
    fn old_layout_with(tier: &str, sources: &[&str]) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch("dx-legacy-layout-");
        let root = scratch.path();
        for source in sources {
            put(root, source, "");
        }
        for rel in [
            "ruff/standard.toml",
            "ruff/strict.toml",
            "eslint/standard.mjs",
            "eslint/strict.mjs",
            "stylelint/standard.json",
            "stylelint/strict.json",
            "tsconfig/standard.json",
            "tsconfig/strict.json",
        ] {
            put(root, &format!(".dx/config/{rel}"), "old\n");
        }
        put(
            root,
            "ruff.toml",
            &format!("line-length = 100\nextend = \".dx/config/ruff/{tier}.toml\"\n"),
        );
        put(root, ESLINT, &eslint(tier));
        put(root, STYLELINT, &stylelint(tier));
        put(
            root,
            "tsconfig.json",
            &format!("{{ \"extends\": \"./.dx/config/tsconfig/{tier}.json\" }}\n"),
        );
        scratch
    }

    fn old_layout(tier: &str) -> dx_test_scratch::TempDir {
        old_layout_with(tier, &["pkg/a.py", "src/a.ts"])
    }

    fn eslint(tier: &str) -> String {
        format!("export {{ default }} from \"./.dx/config/eslint/{tier}.mjs\";\n")
    }

    fn stylelint(tier: &str) -> String {
        format!("{{ \"extends\": [\"./.dx/config/stylelint/{tier}.json\"] }}\n")
    }

    #[test]
    fn a_strict_workspace_is_rewritten_and_keeps_strict() {
        let scratch = old_layout("strict");
        let root = scratch.path();
        put(root, DX_TOML, "schema_version = 1\n\n[families.python]\n");
        let lines = migrate(root).expect("migrate");
        assert_eq!(
            lines,
            [
                "dx: migrated ruff.toml: .dx/config/ruff/strict.toml -> .dx/config/ruff/base.toml",
                "dx: migrated tsconfig.json: .dx/config/tsconfig/strict.json -> .dx/config/tsconfig/base.json",
                "dx: migrated dx.toml: tier = \"strict\"",
            ]
        );
        assert_eq!(
            read(root, "ruff.toml"),
            "line-length = 100\nextend = \".dx/config/ruff/base.toml\"\n"
        );
        assert_eq!(
            read(root, "tsconfig.json"),
            "{ \"extends\": \"./.dx/config/tsconfig/base.json\" }\n"
        );
        assert_eq!(
            read(root, DX_TOML),
            "schema_version = 1\ntier = \"strict\"\n\n[families.python]\n"
        );
        assert_eq!(migrate(root).expect("again"), Vec::<String>::new());
    }

    #[test]
    fn eslint_and_stylelint_stay_on_the_old_file_dx_does_not_render() {
        let scratch = old_layout("strict");
        let root = scratch.path();
        migrate(root).expect("migrate");
        assert_eq!(read(root, ESLINT), eslint("strict"));
        assert_eq!(read(root, STYLELINT), stylelint("strict"));
    }

    #[test]
    fn a_js_only_tsconfig_is_not_rewritten() {
        let scratch = old_layout_with("standard", &["src/a.js"]);
        let root = scratch.path();
        assert_eq!(migrate(root).expect("migrate"), Vec::<String>::new());
        assert!(read(root, "tsconfig.json").contains("tsconfig/standard.json"));
    }

    #[test]
    fn a_standard_workspace_is_rewritten_without_a_tier() {
        let scratch = old_layout("standard");
        let root = scratch.path();
        assert_eq!(migrate(root).expect("migrate").len(), 2);
        assert!(read(root, "ruff.toml").contains("\".dx/config/ruff/base.toml\""));
        assert!(!root.join(DX_TOML).exists());
        assert_eq!(migrate(root).expect("again"), Vec::<String>::new());
    }

    #[test]
    fn a_strict_layer_without_dx_toml_creates_it() {
        let scratch = old_layout("standard");
        let root = scratch.path();
        put(
            root,
            CORRECTNESS_LAYER,
            "common --@rules_dx//config:warnings_as_errors=true\n",
        );
        migrate(root).expect("migrate");
        assert_eq!(
            read(root, DX_TOML),
            "schema_version = 1\ntier = \"strict\"\n"
        );
    }

    #[test]
    fn a_dx_toml_that_does_not_parse_changes_nothing() {
        let scratch = old_layout("strict");
        let root = scratch.path();
        put(root, DX_TOML, "schema_version = = 1\n");
        let error = migrate(root).expect_err("bad dx.toml");
        assert!(error.to_string().contains("dx.toml"), "{error}");
        assert!(read(root, "ruff.toml").contains("ruff/strict.toml"));
        assert_eq!(read(root, DX_TOML), "schema_version = = 1\n");
    }

    #[test]
    fn an_unpinned_old_correctness_layer_gets_a_note() {
        let scratch = old_layout("standard");
        let root = scratch.path();
        put(
            root,
            CORRECTNESS_LAYER,
            "# Generated by dx.\nbuild --keep_going\n",
        );
        let lines = migrate(root).expect("migrate");
        assert_eq!(
            lines.last().map(String::as_str),
            Some(
                "dx: note: the old correctness layer made warnings errors; the tier is now \
                 standard, set tier = \"strict\" in dx.toml to keep errors"
            ),
            "{lines:?}"
        );
        assert!(!root.join(DX_TOML).exists());
    }

    #[test]
    fn a_tier_the_user_set_stays() {
        let scratch = old_layout("strict");
        let root = scratch.path();
        let mine = "schema_version = 1\ntier = \"standard\"\n";
        put(root, DX_TOML, mine);
        let lines = migrate(root).expect("migrate");
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(read(root, DX_TOML), mine);
    }

    #[test]
    fn a_new_layout_workspace_is_left_alone() {
        let scratch = dx_test_scratch::scratch("dx-legacy-layout-");
        let root = scratch.path();
        put(
            root,
            "ruff.toml",
            "extend = \".dx/config/ruff/base.toml\"\n",
        );
        put(
            root,
            CORRECTNESS_LAYER,
            "common --@rules_dx//config:warnings_as_errors=true\n",
        );
        assert_eq!(migrate(root).expect("migrate"), Vec::<String>::new());
        assert!(!root.join(DX_TOML).exists());
    }
}
