pub const DX_WORKSPACE_ENV: &str = "DX_WORKSPACE";
pub const DX_OUTPUT_ENV: &str = "DX_OUTPUT";
pub const DX_VERBOSE_ENV: &str = "DX_VERBOSE";
pub const DX_COLOR_ENV: &str = "DX_COLOR";
pub const DX_QUIET_ENV: &str = "DX_QUIET";
pub const DX_FAIL_ON_ENV: &str = "DX_FAIL_ON";
pub const DX_BUILD_SLOTS_ENV: &str = "DX_BUILD_SLOTS";

/// Environment variables that mark a process started by a coding agent.
pub const AGENT_ENVS: [&str; 5] = [
    "CLAUDECODE",
    "CODEX_THREAD_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "AI_AGENT",
];

/// Whether any agent marker is set to a non-empty value.
pub fn is_agent(get: &dyn Fn(&str) -> Option<String>) -> bool {
    AGENT_ENVS
        .iter()
        .any(|name| env_string(get, name).is_some())
}

/// Environment defaults, the flag each one defaults, and the value shape.
pub const ENV_DEFAULTS: [(&str, &str, &str); 7] = [
    (DX_WORKSPACE_ENV, "--workspace", "dir"),
    (DX_QUIET_ENV, "--quiet", "bool"),
    (DX_VERBOSE_ENV, "--verbose", "bool"),
    (DX_COLOR_ENV, "--color", "mode"),
    (DX_OUTPUT_ENV, "--output", "mode"),
    (DX_FAIL_ON_ENV, "--fail-on", "level"),
    (DX_BUILD_SLOTS_ENV, "--build-slots", "auto|n|off"),
];

/// Spellings that turn a boolean environment default on.
pub const TRUTHY: [&str; 1] = ["true"];

/// Spellings that turn a boolean environment default off.
pub const FALSEY: [&str; 1] = ["false"];

/// Every documented boolean spelling, on spellings first.
pub const BOOL_SPELLINGS: &str = "true|false";

/// Every documented `build-slots` spelling.
pub const BUILD_SLOTS_SPELLINGS: &str = "auto|<n>|off";

/// The largest explicit slot count; each take walks every index up to it.
pub const MAX_BUILD_SLOTS: usize = 1024;

/// How many build slots this dx process sizes its gate with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildSlots {
    /// Size from host memory. The default inside a `dx worktree` slot.
    #[default]
    Auto,
    /// Exactly this many slots, with no memory-based clamp and no remote extras.
    Count(std::num::NonZeroUsize),
    /// Take no slot and pass no `--local_resources`.
    Off,
}

impl BuildSlots {
    /// Read `auto`, `off`, or a count from 1 to `MAX_BUILD_SLOTS`; case and surrounding space are ignored.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "auto" => Some(BuildSlots::Auto),
            "off" => Some(BuildSlots::Off),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => digits
                .parse::<usize>()
                .ok()
                .filter(|count| *count <= MAX_BUILD_SLOTS)
                .and_then(std::num::NonZeroUsize::new)
                .map(BuildSlots::Count),
            _ => None,
        }
    }
}

impl std::fmt::Display for BuildSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildSlots::Auto => f.write_str("auto"),
            BuildSlots::Count(count) => write!(f, "{count}"),
            BuildSlots::Off => f.write_str("off"),
        }
    }
}

/// Read one boolean default, or `None` when the text is not a documented spelling.
pub fn parse_bool(value: &str) -> Option<bool> {
    let value = value.trim().to_ascii_lowercase();
    if TRUTHY.contains(&value.as_str()) {
        Some(true)
    } else if FALSEY.contains(&value.as_str()) {
        Some(false)
    } else {
        None
    }
}

pub fn env_string(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    get(name).filter(|value| !value.is_empty())
}

/// Read one boolean environment default, refusing a value that is neither spelling.
pub fn env_bool(
    get: &dyn Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<Option<bool>, super::AdoptError> {
    let Some(value) = env_string(get, name) else {
        return Ok(None);
    };
    parse_bool(&value)
        .map(Some)
        .ok_or(super::AdoptError::InvalidEnvBool {
            name,
            value,
            spellings: BOOL_SPELLINGS,
        })
}

/// Resolve a string default across flag, environment, then the fallback.
pub fn resolve_string(flag: Option<String>, env: Option<String>, fallback: &str) -> String {
    flag.or(env).unwrap_or_else(|| fallback.to_owned())
}

/// Resolve a boolean default across flag, environment, then off.
pub fn resolve_bool(flag: Option<bool>, env: Option<bool>) -> bool {
    flag.or(env).unwrap_or(false)
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
        for truthy in ["true", "TRUE", " True ", " true"] {
            assert_eq!(parse_bool(truthy), Some(true), "{truthy:?} must enable");
        }
        for falsy in [
            "", "1", "yes", "y", "on", "0", "no", "off", "tru", "2", "maybe",
        ] {
            assert_ne!(parse_bool(falsy), Some(true), "{falsy:?} must not enable");
        }
    }

    #[test]
    fn falsy_spellings_disable_and_anything_else_is_unreadable() {
        for falsy in ["false", "FALSE", " False ", " false"] {
            assert_eq!(parse_bool(falsy), Some(false), "{falsy:?} must disable");
        }
        for on in TRUTHY {
            assert_eq!(parse_bool(on), Some(true), "{on:?} must enable");
        }
        for unreadable in [
            "1", "yes", "y", "on", "0", "no", "n", "off", "", " ", "tru", "ture", "fals", "2",
            "-1", "maybe", "enabled",
        ] {
            assert_eq!(parse_bool(unreadable), None, "{unreadable:?}");
        }
        assert_eq!(BOOL_SPELLINGS, "true|false");
    }

    #[test]
    fn env_bool_refuses_a_value_outside_the_documented_spellings() {
        let get = env_of(&[(DX_QUIET_ENV, "tru")]);
        let error = env_bool(&get, DX_QUIET_ENV).expect_err("typo is not a boolean");
        assert_eq!(
            error,
            super::super::AdoptError::InvalidEnvBool {
                name: DX_QUIET_ENV,
                value: "tru".to_owned(),
                spellings: BOOL_SPELLINGS,
            }
        );
        assert_eq!(
            error.to_string(),
            "invalid invocation default DX_QUIET=\"tru\": want one of true|false"
        );
        for (name, value, wanted) in [
            (DX_QUIET_ENV, "false", false),
            (DX_VERBOSE_ENV, "FALSE", false),
            (DX_VERBOSE_ENV, "true", true),
            (DX_QUIET_ENV, " True ", true),
        ] {
            let pairs = [(name, value)];
            let get = env_of(&pairs);
            assert_eq!(
                env_bool(&get, name).expect("documented spelling"),
                Some(wanted)
            );
        }
        let get = env_of(&[(DX_VERBOSE_ENV, ""), (DX_QUIET_ENV, "true")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(None));
        assert_eq!(env_bool(&get, DX_QUIET_ENV), Ok(Some(true)));
        assert_eq!(env_bool(&env_of(&[]), DX_VERBOSE_ENV), Ok(None));
    }

    #[test]
    fn env_defaults_cover_every_documented_variable() {
        assert_eq!(
            ENV_DEFAULTS.map(|(env, _, _)| env),
            [
                DX_WORKSPACE_ENV,
                DX_QUIET_ENV,
                DX_VERBOSE_ENV,
                DX_COLOR_ENV,
                DX_OUTPUT_ENV,
                DX_FAIL_ON_ENV,
                DX_BUILD_SLOTS_ENV
            ]
        );
        assert_eq!(TRUTHY, ["true"]);
        assert_eq!(FALSEY, ["false"]);
    }

    #[test]
    fn env_helpers_ignore_missing_and_empty_strings() {
        let get = env_of(&[(DX_WORKSPACE_ENV, "/repo"), (DX_OUTPUT_ENV, "")]);
        assert_eq!(env_string(&get, DX_WORKSPACE_ENV), Some("/repo".to_owned()));
        assert_eq!(env_string(&get, DX_OUTPUT_ENV), None);
        assert_eq!(env_string(&get, DX_VERBOSE_ENV), None);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(None));
        let get = env_of(&[(DX_VERBOSE_ENV, "true")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(Some(true)));
        let get = env_of(&[(DX_VERBOSE_ENV, "false")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(Some(false)));
        assert_eq!(env_bool(&env_of(&[]), DX_QUIET_ENV), Ok(None));
    }

    #[test]
    fn precedence_resolves_flag_over_env() {
        assert_eq!(
            resolve_string(Some("flag".to_owned()), Some("env".to_owned()), "fallback"),
            "flag"
        );
        assert_eq!(
            resolve_string(None, Some("env".to_owned()), "fallback"),
            "env"
        );
        assert_eq!(resolve_string(None, None, "fallback"), "fallback");
        assert!(resolve_bool(Some(true), Some(false)));
        assert!(resolve_bool(None, Some(true)));
        assert!(!resolve_bool(Some(false), Some(true)));
        assert!(!resolve_bool(None, None));
    }

    #[test]
    fn build_slots_parse_accepts_only_auto_off_and_counts() {
        let count = |n: usize| BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"));
        for (text, want) in [
            ("auto", Some(BuildSlots::Auto)),
            ("AUTO", Some(BuildSlots::Auto)),
            (" off ", Some(BuildSlots::Off)),
            ("2", Some(count(2))),
            ("1024", Some(count(1024))),
            ("0", None),
            ("-1", None),
            ("+2", None),
            ("many", None),
            ("2.5", None),
            ("1025", None),
            ("", None),
        ] {
            assert_eq!(BuildSlots::parse(text), want, "{text:?}");
        }
        for setting in [BuildSlots::Auto, BuildSlots::Off, count(7)] {
            assert_eq!(BuildSlots::parse(&setting.to_string()), Some(setting));
        }
    }
}
