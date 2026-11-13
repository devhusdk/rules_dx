use super::command::Command;

/// The page under `docs/cli/commands/` that documents `dx` itself.
pub const ROOT_PAGE: &str = "README.md";

/// One flag `--help` lists: its long name, or its short one when it has no long name, and its other spellings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HelpFlag {
    pub name: String,
    pub aliases: Vec<String>,
}

/// What `--help` says about `dx` (an empty path), one command or one subcommand, and the page that documents it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpSurface {
    pub path: Vec<String>,
    pub page: Option<&'static str>,
    pub flags: Vec<HelpFlag>,
    pub exit_codes: Vec<String>,
}

impl Command {
    /// The page under `docs/cli/commands/` that documents this command.
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
        }
    }
}

/// The numbers on the `Exit codes:` line of one rendered help text.
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
            })
        })
        .collect();
    flags.sort();
    flags.dedup();
    flags
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
            exit_codes: exit_codes_in(&sub.clone().render_long_help().to_string()),
        });
        collect(sub, &sub_path, sub_page, out);
    }
}

/// `dx` itself, then every command and subcommand the grammar knows.
pub fn help_surface() -> Vec<HelpSurface> {
    let mut root = super::grammar::cli_command();
    root.build();
    let mut out = vec![HelpSurface {
        path: Vec::new(),
        page: Some(ROOT_PAGE),
        flags: visible_flags(&root),
        exit_codes: Vec::new(),
    }];
    collect(&root, &[], None, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::{Command, ROOT_PAGE, exit_codes_in, help_surface};

    #[test]
    fn every_command_names_its_page() {
        assert_eq!(Command::Coverage.page(), "build-test-coverage.md");
        assert_eq!(Command::License.page(), "audit-update.md");
    }

    #[test]
    fn exit_codes_are_read_from_the_exit_codes_line_only() {
        let help = "Usage: dx watch\n  --jobs 4\nExit codes: 2 usage errors; 1 failures; 141 when stdout closes; 2 again.\n";
        assert_eq!(exit_codes_in(help), ["2", "1", "141"]);
        assert!(exit_codes_in("Usage: dx 7\n").is_empty());
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
            flag(test, "--output").is_some_and(|output| output.aliases == ["-o"]),
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
