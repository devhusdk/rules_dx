use super::command::Command;

/// The page under `docs/cli/commands/` that documents `dx` itself.
pub const ROOT_PAGE: &str = "README.md";

/// One flag `--help` lists: its long name, or its short one when it has no long name, its other spellings,
/// the values it accepts when they are a fixed set, and its defaults.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HelpFlag {
    pub name: String,
    pub aliases: Vec<String>,
    pub values: Vec<String>,
    pub defaults: Vec<String>,
}

/// One positional argument `--help` lists, in order: its value name, whether it is required, whether it takes
/// more than one word, the values it accepts when they are a fixed set, and its defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpPositional {
    pub name: String,
    pub required: bool,
    pub multiple: bool,
    pub values: Vec<String>,
    pub defaults: Vec<String>,
}

/// One environment variable `dx --help` documents: its name, the shape of its value, and the flag it defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEnv {
    pub name: String,
    pub shape: String,
    pub default_for: Option<String>,
}

/// What `--help` says about `dx` (an empty path), one command or one subcommand, and the page that documents it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpSurface {
    pub path: Vec<String>,
    pub page: Option<&'static str>,
    pub flags: Vec<HelpFlag>,
    pub positionals: Vec<HelpPositional>,
    pub exit_codes: Vec<String>,
}

impl Command {
    /// The page under `docs/cli/commands/` that documents this command.
    #[must_use]
    pub fn page(self) -> &'static str {
        match self {
            Command::Security | Command::License | Command::Update => "audit-update.md",
            Command::Lint | Command::Typecheck | Command::Format => "quality.md",
            Command::Generate => "generate.md",
            Command::Build | Command::Test | Command::Coverage | Command::Run => {
                "build-test-coverage.md"
            }
            Command::Bench => "bench.md",
            Command::Deploy => "deploy.md",
            Command::Check => "check.md",
            Command::Clean => "clean.md",
            Command::Sync => "sync.md",
            Command::Init => "init.md",
            Command::Upgrade => "upgrade.md",
            Command::Status => "status.md",
            Command::Watch => "watch.md",
            Command::Completion => "completion.md",
            Command::Docs => "docs.md",
            Command::Worktree => "worktree.md",
            Command::Fuzz => "fuzz.md",
            Command::Config => "config.md",
            Command::Fetch => "fetch.md",
        }
    }
}

/// The numbers on the `Exit codes:` line of one rendered help text.
#[must_use]
pub fn exit_codes_in(help: &str) -> Vec<String> {
    let mut codes = Vec::new();
    for line in help.lines() {
        let Some(rest) = line.trim_start().strip_prefix("Exit codes:") else {
            continue;
        };
        for word in rest.split(|c: char| !c.is_ascii_digit()) {
            if !word.is_empty() && !codes.iter().any(|code| code == word) {
                codes.push(word.to_owned());
            }
        }
    }
    codes
}

fn visible_flags(command: &clap::Command) -> Vec<HelpFlag> {
    let mut flags: Vec<HelpFlag> = command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set() && arg.get_id() != "help")
        .filter_map(|arg| {
            let mut spellings: Vec<String> = Vec::new();
            spellings.extend(arg.get_long().map(|long| format!("--{long}")));
            spellings.extend(arg.get_short().map(|short| format!("-{short}")));
            for alias in arg.get_all_aliases().unwrap_or_default() {
                spellings.push(format!("--{alias}"));
            }
            for alias in arg.get_all_short_aliases().unwrap_or_default() {
                spellings.push(format!("-{alias}"));
            }
            let mut spellings = spellings.into_iter();
            spellings.next().map(|name| HelpFlag {
                name,
                aliases: spellings.collect(),
                values: accepted_values(arg),
                defaults: default_values(arg),
            })
        })
        .collect();
    flags.sort();
    flags.dedup();
    flags
}

// Switches carry clap's implicit `true`/`false` values and `false` default; they say nothing a user
// types, so only arguments that take a value report them.
fn accepted_values(arg: &clap::Arg) -> Vec<String> {
    if !arg.get_action().takes_values() {
        return Vec::new();
    }
    arg.get_possible_values()
        .iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect()
}

fn default_values(arg: &clap::Arg) -> Vec<String> {
    if !arg.get_action().takes_values() {
        return Vec::new();
    }
    arg.get_default_values()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect()
}

fn visible_positionals(command: &clap::Command) -> Vec<HelpPositional> {
    command
        .get_positionals()
        .filter(|arg| !arg.is_hide_set())
        .map(|arg| HelpPositional {
            name: arg
                .get_value_names()
                .and_then(|names| names.first())
                .map_or_else(
                    || arg.get_id().as_str().to_ascii_uppercase(),
                    ToString::to_string,
                ),
            required: arg.is_required_set(),
            multiple: arg
                .get_num_args()
                .is_some_and(|range| range.max_values() > 1),
            values: accepted_values(arg),
            defaults: default_values(arg),
        })
        .collect()
}

/// Every environment variable `dx --help` documents, `DX_` flag defaults first.
#[must_use]
pub fn help_environment() -> Vec<HelpEnv> {
    let defaults = dx_adopt::defaults::ENV_DEFAULTS
        .into_iter()
        .map(|(name, flag, shape)| HelpEnv {
            name: name.to_owned(),
            shape: shape.to_owned(),
            default_for: Some(flag.to_owned()),
        });
    let others = super::help::OTHER_ENV
        .into_iter()
        .map(|(name, shape, _)| HelpEnv {
            name: name.to_owned(),
            shape: shape.to_owned(),
            default_for: None,
        });
    defaults.chain(others).collect()
}

fn collect(
    command: &clap::Command,
    path: &[String],
    page: Option<&'static str>,
    out: &mut Vec<HelpSurface>,
) {
    for sub in command.get_subcommands() {
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }
        let mut sub_path = path.to_vec();
        sub_path.push(sub.get_name().to_owned());
        let sub_page = if path.is_empty() {
            Command::parse(sub.get_name()).map(Command::page)
        } else {
            page
        };
        out.push(HelpSurface {
            path: sub_path.clone(),
            page: sub_page,
            flags: visible_flags(sub),
            positionals: visible_positionals(sub),
            exit_codes: exit_codes_in(&sub.clone().render_long_help().to_string()),
        });
        collect(sub, &sub_path, sub_page, out);
    }
}

/// `dx` itself, then every command and subcommand the grammar knows.
#[must_use]
pub fn help_surface() -> Vec<HelpSurface> {
    let mut root = super::grammar::cli_command();
    root.build();
    let mut out = vec![HelpSurface {
        path: Vec::new(),
        page: Some(ROOT_PAGE),
        flags: visible_flags(&root),
        positionals: visible_positionals(&root),
        exit_codes: Vec::new(),
    }];
    collect(&root, &[], None, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::{Command, ROOT_PAGE, exit_codes_in, help_environment, help_surface};

    #[test]
    fn every_command_names_its_page() {
        assert_eq!(Command::Coverage.page(), "build-test-coverage.md");
        assert_eq!(Command::License.page(), "audit-update.md");
    }

    #[test]
    fn exit_codes_are_read_from_the_exit_codes_line_only() {
        let help = "Usage: dx watch\n  --jobs 4\nExit codes: 2 usage errors; 1 failures; 141 when stdout closes; 2 again.\n";
        assert_eq!(exit_codes_in(help), ["2", "1", "141"]);
        assert_eq!(exit_codes_in("Usage: dx 7\n"), [] as [String; 0]);
    }

    #[test]
    fn the_environment_matches_the_top_help() {
        let help = super::super::help::render_top_help();
        let environment = help_environment();
        for env in &environment {
            assert!(
                help.contains(&format!("  {}=<{}>", env.name, env.shape)),
                "{env:?} not in top help"
            );
        }
        assert!(
            environment.iter().any(
                |env| env.name == "DX_OUTPUT" && env.default_for.as_deref() == Some("--output")
            )
        );
        assert!(
            environment
                .iter()
                .any(|env| env.name == "NO_COLOR" && env.default_for.is_none())
        );
    }

    #[test]
    fn the_surface_lists_commands_subcommands_and_their_flags() {
        let surface = help_surface();
        let find = |path: &[&str]| {
            surface
                .iter()
                .find(|entry| entry.path == path)
                .unwrap_or_else(|| panic!("no entry for {path:?}"))
        };
        let flag = |entry: &super::HelpSurface, name: &str| {
            entry.flags.iter().find(|flag| flag.name == name).cloned()
        };
        let root = find(&[]);
        assert_eq!(root.page, Some(ROOT_PAGE));
        assert!(flag(root, "--version").is_some(), "{root:?}");
        let test = find(&["test"]);
        assert_eq!(test.page, Some("build-test-coverage.md"));
        assert!(flag(test, "--affected").is_some(), "{test:?}");
        assert!(
            flag(test, "--output")
                .is_some_and(|output| output.aliases == ["-o"] && output.values == ["text", "json"]),
            "{test:?}"
        );
        assert!(
            flag(test, "--affected").is_some_and(|affected| affected.values.is_empty()),
            "{test:?}"
        );
        assert!(
            flag(test, "--help").is_none() && flag(test, "-o").is_none(),
            "{test:?}"
        );
        assert_eq!(test.exit_codes, ["0", "2", "1"]);
        for entry in surface.iter().filter(|entry| entry.path.len() > 1) {
            assert_eq!(
                entry.page,
                find(&[entry.path[0].as_str()]).page,
                "{entry:?}"
            );
        }
        for &command in <Command as clap::ValueEnum>::value_variants() {
            assert_eq!(find(&[command.name()]).page, Some(command.page()));
        }
    }
}
