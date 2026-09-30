use std::path::{Path, PathBuf};

pub const DX_WORKSPACE_ENV: &str = "DX_WORKSPACE";
pub const DX_OUTPUT_ENV: &str = "DX_OUTPUT";
pub const DX_VERBOSE_ENV: &str = "DX_VERBOSE";
pub const DX_COLOR_ENV: &str = "DX_COLOR";
pub const DX_QUIET_ENV: &str = "DX_QUIET";
pub const DX_DRY_RUN_ENV: &str = "DX_DRY_RUN";
pub const DX_FAIL_ON_ENV: &str = "DX_FAIL_ON";

/// Environment defaults, the flag each one defaults, and the value shape.
pub const ENV_DEFAULTS: [(&str, &str, &str); 7] = [
    (DX_WORKSPACE_ENV, "--workspace", "dir"),
    (DX_DRY_RUN_ENV, "--dry-run", "bool"),
    (DX_QUIET_ENV, "--quiet", "bool"),
    (DX_VERBOSE_ENV, "--verbose", "bool"),
    (DX_COLOR_ENV, "--color", "mode"),
    (DX_OUTPUT_ENV, "--output", "mode"),
    (DX_FAIL_ON_ENV, "--fail-on", "level"),
];

/// Spellings that turn a boolean environment default on.
pub const TRUTHY: [&str; 5] = ["1", "true", "yes", "y", "on"];

/// The `.dx/config.toml` key that carries the same default as an env var.
pub fn config_key(env: &str) -> Option<&'static str> {
    ENV_DEFAULTS
        .iter()
        .find(|(name, _, _)| *name == env)
        .map(|(_, flag, _)| flag.trim_start_matches("--"))
}

pub const CONFIG_TOML_REL: &str = ".dx/config.toml";
pub const CONFIG_REL: &str = ".dx/config";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDefaults {
    pub workspace: Option<String>,
    pub output: Option<String>,
    pub verbose: Option<bool>,
    pub color: Option<String>,
    pub quiet: Option<bool>,
    pub dry_run: Option<bool>,
    pub fail_on: Option<String>,
}

pub fn is_truthy(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    TRUTHY.contains(&value.as_str())
}

pub fn env_string(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    get(name).filter(|value| !value.is_empty())
}

pub fn env_bool(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<bool> {
    get(name).map(|value| is_truthy(&value))
}

pub fn resolve_string(
    flag: Option<String>,
    env: Option<String>,
    file: Option<String>,
    fallback: &str,
) -> String {
    flag.or(env).or(file).unwrap_or_else(|| fallback.to_owned())
}

pub fn resolve_workspace(
    flag: Option<String>,
    env: Option<String>,
    file: Option<String>,
) -> Option<String> {
    flag.or(env).or(file)
}

pub fn resolve_bool(flag: bool, env: Option<bool>, file: Option<bool>) -> bool {
    if flag {
        return true;
    }
    env.or(file).unwrap_or(false)
}

#[derive(Debug, Default, serde::Deserialize)]
struct DxTable {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    quiet: Option<bool>,
    #[serde(default, alias = "dry-run")]
    dry_run: Option<bool>,
    #[serde(default, alias = "fail-on")]
    fail_on: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct ConfigFile {
    #[serde(default)]
    dx: Option<DxTable>,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    quiet: Option<bool>,
    #[serde(default, alias = "dry-run")]
    dry_run: Option<bool>,
    #[serde(default, alias = "fail-on")]
    fail_on: Option<String>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

pub fn parse_file_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    let parsed: ConfigFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    let table = parsed.dx.unwrap_or_default();
    Ok(FileDefaults {
        workspace: non_empty(table.workspace.or(parsed.workspace)),
        output: non_empty(table.output.or(parsed.output)),
        verbose: table.verbose.or(parsed.verbose),
        color: non_empty(table.color.or(parsed.color)),
        quiet: table.quiet.or(parsed.quiet),
        dry_run: table.dry_run.or(parsed.dry_run),
        fail_on: non_empty(table.fail_on.or(parsed.fail_on)),
    })
}

pub fn find_config(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let toml = dir.join(CONFIG_TOML_REL);
        if toml.is_file() {
            return Some(toml);
        }
        let plain = dir.join(CONFIG_REL);
        if plain.is_file() {
            return Some(plain);
        }
    }
    None
}

pub fn load_defaults(start: &Path) -> Result<(FileDefaults, Option<PathBuf>), super::AdoptError> {
    let Some(path) = find_config(start) else {
        return Ok((FileDefaults::default(), None));
    };
    let text = std::fs::read_to_string(&path).map_err(|e| super::AdoptError::InvalidDefaults {
        detail: format!("cannot read {}: {e}", path.display()),
    })?;
    let defaults = parse_file_text(&text).map_err(|e| match e {
        super::AdoptError::InvalidDefaults { detail } => super::AdoptError::InvalidDefaults {
            detail: format!("{}: {detail}", path.display()),
        },
        other => other,
    })?;
    Ok((defaults, Some(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn truthy_spellings_enable_only_documented_values() {
        for truthy in [
            "1", "true", "TRUE", " True ", "yes", "YES", "y", "Y", "on", "ON",
        ] {
            assert!(is_truthy(truthy), "{truthy:?} must enable");
        }
        for falsy in ["", "0", "false", "no", "off", "tru", "2", "maybe"] {
            assert!(!is_truthy(falsy), "{falsy:?} must not enable");
        }
    }

    #[test]
    fn env_defaults_cover_every_documented_variable() {
        assert_eq!(
            ENV_DEFAULTS.map(|(env, _, _)| env),
            [
                DX_WORKSPACE_ENV,
                DX_DRY_RUN_ENV,
                DX_QUIET_ENV,
                DX_VERBOSE_ENV,
                DX_COLOR_ENV,
                DX_OUTPUT_ENV,
                DX_FAIL_ON_ENV
            ]
        );
        assert_eq!(TRUTHY, ["1", "true", "yes", "y", "on"]);
        for (env, flag, _) in ENV_DEFAULTS {
            assert_eq!(
                config_key(env),
                Some(flag.trim_start_matches("--")),
                "{env}"
            );
        }
        assert_eq!(config_key("DX_NOPE"), None);
    }

    #[test]
    fn env_helpers_ignore_missing_and_empty_strings() {
        let get = env_of(&[(DX_WORKSPACE_ENV, "/repo"), (DX_OUTPUT_ENV, "")]);
        assert_eq!(env_string(&get, DX_WORKSPACE_ENV), Some("/repo".to_owned()));
        assert_eq!(env_string(&get, DX_OUTPUT_ENV), None);
        assert_eq!(env_string(&get, DX_VERBOSE_ENV), None);
        assert_eq!(env_bool(&get, DX_WORKSPACE_ENV), Some(false));
        let get = env_of(&[(DX_VERBOSE_ENV, "yes")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Some(true));
        let get = env_of(&[(DX_VERBOSE_ENV, "0")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Some(false));
        assert_eq!(env_bool(&get, DX_QUIET_ENV), None);
    }

    #[test]
    fn precedence_resolves_flag_over_env_over_file() {
        assert_eq!(
            resolve_string(
                Some("flag".to_owned()),
                Some("env".to_owned()),
                Some("file".to_owned()),
                "fallback"
            ),
            "flag"
        );
        assert_eq!(
            resolve_string(
                None,
                Some("env".to_owned()),
                Some("file".to_owned()),
                "fallback"
            ),
            "env"
        );
        assert_eq!(
            resolve_string(None, None, Some("file".to_owned()), "fallback"),
            "file"
        );
        assert_eq!(resolve_string(None, None, None, "fallback"), "fallback");
        assert_eq!(
            resolve_workspace(
                Some("flag".to_owned()),
                Some("env".to_owned()),
                Some("file".to_owned())
            ),
            Some("flag".to_owned())
        );
        assert_eq!(resolve_workspace(None, None, None), None);
        assert!(resolve_bool(true, Some(false), Some(false)));
        assert!(resolve_bool(false, Some(true), Some(false)));
        assert!(resolve_bool(false, Some(false), Some(true)) == false);
        assert!(resolve_bool(false, None, Some(true)));
        assert!(!resolve_bool(false, None, None));
    }

    #[test]
    fn file_parses_dx_table_with_top_level_alias() {
        let parsed = parse_file_text(
            "[dx]\nworkspace = \"/repo\"\noutput = \"json\"\nverbose = true\ncolor = \"never\"\nquiet = false\ndry_run = true\nfail_on = \"error\"\n",
        )
        .expect("dx table parses");
        assert_eq!(parsed.workspace, Some("/repo".to_owned()));
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert_eq!(parsed.verbose, Some(true));
        assert_eq!(parsed.color, Some("never".to_owned()));
        assert_eq!(parsed.quiet, Some(false));
        assert_eq!(parsed.dry_run, Some(true));
        assert_eq!(parsed.fail_on, Some("error".to_owned()));
        let top = parse_file_text("workspace = \"/top\"\nverbose = true\n").expect("top parses");
        assert_eq!(top.workspace, Some("/top".to_owned()));
        assert_eq!(top.verbose, Some(true));
        let both =
            parse_file_text("output = \"text\"\n[dx]\noutput = \"json\"\n").expect("table wins");
        assert_eq!(both.output, Some("json".to_owned()));
        let both_color =
            parse_file_text("color = \"never\"\n[dx]\ncolor = \"always\"\n").expect("table wins");
        assert_eq!(both_color.color, Some("always".to_owned()));
        let empty = parse_file_text("").expect("empty parses");
        assert_eq!(empty, FileDefaults::default());
        assert!(parse_file_text("not toml = [").is_err());
        assert!(parse_file_text("[dx]\nverbose = \"yes\"\n").is_err());
    }

    #[test]
    fn file_hyphen_aliases_and_empty_strings_are_absent() {
        let parsed = parse_file_text("[dx]\n\"dry-run\" = true\n\"fail-on\" = \"info\"\n")
            .expect("hyphen aliases parse");
        assert_eq!(parsed.dry_run, Some(true));
        assert_eq!(parsed.fail_on, Some("info".to_owned()));
        let parsed = parse_file_text("workspace = \"\"\noutput = \"\"\ncolor = \"\"\n")
            .expect("empty parses");
        assert_eq!(parsed.workspace, None);
        assert_eq!(parsed.output, None);
        assert_eq!(parsed.color, None);
    }

    #[test]
    fn find_and_load_prefers_toml_and_walks_up() {
        let scratch = dx_test_scratch::scratch("dx-defaults-");
        let root = scratch.path().to_path_buf();
        let sub = root.join("sub/dir");
        std::fs::create_dir_all(sub.join(".dx")).expect("dirs");
        std::fs::create_dir_all(root.join(".dx")).expect("root dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("root config");
        std::fs::write(sub.join(".dx/config.toml"), "[dx]\noutput = \"diff\"\n")
            .expect("sub config");
        assert_eq!(
            find_config(&sub),
            Some(sub.join(".dx/config.toml")),
            "nearest file wins"
        );
        let (defaults, path) = load_defaults(&sub).expect("loads nearest");
        assert_eq!(defaults.output, Some("diff".to_owned()));
        assert_eq!(path, Some(sub.join(".dx/config.toml")));
        std::fs::remove_file(sub.join(".dx/config.toml")).expect("remove sub");
        let (defaults, _) = load_defaults(&sub).expect("falls back upward");
        assert_eq!(defaults.output, Some("json".to_owned()));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn load_missing_is_empty_and_invalid_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-defaults-missing-");
        let root = scratch.path().to_path_buf();
        let (defaults, path) = load_defaults(&root).expect("missing is empty");
        assert_eq!(defaults, FileDefaults::default());
        assert_eq!(path, None);
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "not toml = [").expect("bad config");
        assert!(load_defaults(&root).is_err());
        scratch.close().expect("cleanup");
    }
}
