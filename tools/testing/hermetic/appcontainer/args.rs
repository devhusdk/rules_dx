//! Command line of the Windows launcher:
//! `--name N [--backend appcontainer|user] [--net] [--group G] [--grant P]... [--deny P]... -- cmd args`
//! runs a command, and `--backend user --setup-group G|--teardown-group G [--grant P]... [--deny P]...`
//! adds or removes a local group's entries on those paths, so runs that join it with
//! `--group G` need no ACL changes of their own.
#![cfg_attr(test, allow(dead_code))]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    AppContainer,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Run,
    SetupGroup,
    TeardownGroup,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub name: String,
    pub backend: Backend,
    pub net: bool,
    pub group: Option<String>,
    pub grants: Vec<String>,
    pub denies: Vec<String>,
    pub command: Vec<String>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

pub fn parse(args: &[String]) -> Result<Config, String> {
    let mut name = None;
    let mut net = false;
    let mut backend = Backend::AppContainer;
    let mut member_of = None;
    let mut managed = None;
    let mut grants = Vec::new();
    let mut denies = Vec::new();
    let mut command = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--name" => name = Some(iter.next().ok_or("--name needs a value")?.clone()),
            "--grant" => grants.push(iter.next().ok_or("--grant needs a path")?.clone()),
            "--deny" => denies.push(iter.next().ok_or("--deny needs a path")?.clone()),
            "--group" => member_of = Some(iter.next().ok_or("--group needs a name")?.clone()),
            "--setup-group" | "--teardown-group" => {
                if managed.is_some() {
                    return Err("only one of --setup-group and --teardown-group".to_owned());
                }
                let mode = if arg == "--setup-group" {
                    Mode::SetupGroup
                } else {
                    Mode::TeardownGroup
                };
                let group = iter.next().ok_or(format!("{arg} needs a name"))?.clone();
                managed = Some((mode, group));
            }
            "--backend" => {
                backend = match iter.next().ok_or("--backend needs a value")?.as_str() {
                    "appcontainer" => Backend::AppContainer,
                    "user" => Backend::User,
                    other => return Err(format!("unknown backend {other}")),
                }
            }
            "--net" => net = true,
            "--" => {
                command = Some(iter.by_ref().cloned().collect::<Vec<String>>());
                break;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if let Some((mode, group)) = managed {
        if backend != Backend::User {
            return Err("group setup and teardown need --backend user".to_owned());
        }
        if member_of.is_some() || command.is_some() || name.is_some() || net {
            return Err("group setup and teardown take only --grant and --deny".to_owned());
        }
        if !valid_name(&group) {
            return Err(format!("invalid group name {group:?}"));
        }
        return Ok(Config {
            mode,
            name: String::new(),
            backend,
            net,
            group: Some(group),
            grants,
            denies,
            command: Vec::new(),
        });
    }
    let command = command.ok_or("missing -- before the command")?;
    if command.is_empty() {
        return Err("no command after --".to_owned());
    }
    let name = name.ok_or("--name is required")?;
    if !valid_name(&name) {
        return Err(format!("invalid container name {name:?}"));
    }
    if let Some(group) = &member_of {
        if backend != Backend::User {
            return Err("--group needs --backend user".to_owned());
        }
        if !valid_name(group) {
            return Err(format!("invalid group name {group:?}"));
        }
    }
    Ok(Config {
        mode: Mode::Run,
        name,
        backend,
        net,
        group: member_of,
        grants,
        denies,
        command,
    })
}

/// Directories above the grants, from the drive root down, that are not granted themselves.
/// The container gets list and traverse rights on these so it can resolve the granted paths.
pub fn ancestors(grants: &[String]) -> Result<Vec<String>, String> {
    let mut granted = Vec::new();
    for grant in grants {
        let path = grant.replace('/', "\\");
        let parts: Vec<&str> = path.split('\\').filter(|part| !part.is_empty()).collect();
        let drive = parts.first().map_or(&[][..], |part| part.as_bytes());
        if path.starts_with('\\')
            || drive.len() != 2
            || !drive[0].is_ascii_alphabetic()
            || drive[1] != b':'
        {
            return Err(format!("grant {grant:?} is not a drive letter path"));
        }
        granted.push(
            parts
                .iter()
                .map(|part| part.to_owned())
                .collect::<Vec<_>>()
                .join("\\"),
        );
    }
    let covered = |dir: &str| {
        let dir = dir.trim_end_matches('\\').to_lowercase();
        granted.iter().any(|grant| {
            let grant = grant.to_lowercase();
            dir == grant || dir.starts_with(&format!("{grant}\\"))
        })
    };
    let mut out: Vec<String> = Vec::new();
    for path in &granted {
        let parts: Vec<&str> = path.split('\\').collect();
        for depth in 1..parts.len() {
            let dir = if depth == 1 {
                format!("{}\\", parts[0])
            } else {
                parts[..depth].join("\\")
            };
            if !covered(&dir) && !out.iter().any(|have| have.eq_ignore_ascii_case(&dir)) {
                out.push(dir);
            }
        }
    }
    Ok(out)
}

/// A local account name of at most 20 characters, stable for one container name.
pub fn user_name(name: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("dxh-{hash:016x}")
}

/// The pid in a run's group name `dxh-<pid>`, or `None` for any other name.
pub fn group_pid(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("dxh-")?;
    if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The `dxh-<pid>` groups, other than `own`, whose process is gone.
pub fn stale_groups(names: &[String], own: &str, alive: impl Fn(u32) -> bool) -> Vec<String> {
    names
        .iter()
        .filter(|name| name.as_str() != own)
        .filter(|name| group_pid(name).is_some_and(|pid| !alive(pid)))
        .cloned()
        .collect()
}

/// A password built from random bytes that has upper case, lower case, digits and symbols.
pub fn password(bytes: &[u8]) -> String {
    const CLASSES: [&str; 4] = [
        "ABCDEFGHJKLMNPQRSTUVWXYZ",
        "abcdefghijkmnpqrstuvwxyz",
        "23456789",
        "!#$%&*+-=?@^_",
    ];
    bytes
        .iter()
        .enumerate()
        .map(|(at, byte)| {
            let class = CLASSES[at % CLASSES.len()].as_bytes();
            class[usize::from(*byte) % class.len()] as char
        })
        .collect()
}

/// Joins arguments into one Windows command line, quoted for CommandLineToArgvW.
pub fn command_line(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The command line for CreateProcessWithLogonW, which rejects lines over 1024 characters
/// with only "The parameter is incorrect".
pub fn logon_command_line(command: &[String]) -> Result<String, String> {
    let line = command_line(command);
    if line.encode_utf16().count() > 1024 {
        return Err(format!(
            "the command line of {} is longer than the 1024 characters the run account accepts",
            command[0]
        ));
    }
    Ok(line)
}

fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

/// Variables Windows needs to create an AppContainer process.
pub const ESSENTIAL: [&str; 11] = [
    "SystemRoot",
    "windir",
    "SystemDrive",
    "ComSpec",
    "PATHEXT",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "ProgramData",
    "TEMP",
    "TMP",
];

/// The parent's variables plus any missing ESSENTIAL ones taken from `defaults`,
/// as a sorted UTF-16 block, with the names that were added.
pub fn environment(
    parent: &[(String, String)],
    defaults: &[(String, String)],
) -> (Vec<u16>, Vec<String>) {
    let mut vars: Vec<(String, String)> = parent.to_vec();
    let mut added = Vec::new();
    for name in ESSENTIAL {
        if vars.iter().any(|(have, _)| have.eq_ignore_ascii_case(name)) {
            continue;
        }
        if let Some((_, value)) = defaults
            .iter()
            .find(|(have, _)| have.eq_ignore_ascii_case(name))
        {
            vars.push((name.to_owned(), value.clone()));
            added.push(name.to_owned());
        }
    }
    vars.sort_by_key(|(name, _)| name.to_uppercase());
    let mut block = Vec::new();
    for (name, value) in vars {
        block.extend(format!("{name}={value}").encode_utf16());
        block.push(0);
    }
    block.push(0);
    (block, added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn parses_flags_grants_and_command() {
        let config = parse(&strings(&[
            "--name", "run-1", "--net", "--grant", "C:\\a b", "--grant", "C:\\c", "--", "cmd.exe",
            "/c", "--net",
        ]))
        .unwrap();
        assert_eq!(config.name, "run-1");
        assert!(config.net);
        assert_eq!(config.grants, ["C:\\a b", "C:\\c"]);
        assert_eq!(config.command, ["cmd.exe", "/c", "--net"]);
    }

    #[test]
    fn backend_and_deny_are_parsed() {
        let config = parse(&strings(&[
            "--name",
            "n",
            "--backend",
            "user",
            "--deny",
            "C:\\x",
            "--",
            "y",
        ]))
        .unwrap();
        assert_eq!(config.backend, Backend::User);
        assert_eq!(config.denies, ["C:\\x"]);
        let default = parse(&strings(&["--name", "n", "--", "y"])).unwrap();
        assert_eq!(default.backend, Backend::AppContainer);
        assert!(parse(&strings(&["--name", "n", "--backend", "x", "--", "y"])).is_err());
    }

    #[test]
    fn a_run_can_join_a_group() {
        let config = parse(&strings(&[
            "--name",
            "n",
            "--backend",
            "user",
            "--group",
            "dxh-7",
            "--",
            "y",
        ]))
        .unwrap();
        assert_eq!(config.mode, Mode::Run);
        assert_eq!(config.group.as_deref(), Some("dxh-7"));
        assert!(parse(&strings(&["--name", "n", "--group", "dxh-7", "--", "y"])).is_err());
        assert!(
            parse(&strings(&[
                "--name",
                "n",
                "--backend",
                "user",
                "--group",
                "a b",
                "--",
                "y"
            ]))
            .is_err()
        );
    }

    #[test]
    fn group_setup_and_teardown_take_paths_and_no_command() {
        let setup = parse(&strings(&[
            "--backend",
            "user",
            "--setup-group",
            "dxh-7",
            "--grant",
            "C:\\q",
            "--deny",
            "C:\\x",
        ]))
        .unwrap();
        assert_eq!(setup.mode, Mode::SetupGroup);
        assert_eq!(setup.group.as_deref(), Some("dxh-7"));
        assert_eq!(setup.grants, ["C:\\q"]);
        assert_eq!(setup.denies, ["C:\\x"]);
        assert!(setup.command.is_empty());
        let teardown = parse(&strings(&[
            "--backend",
            "user",
            "--teardown-group",
            "dxh-7",
            "--grant",
            "C:\\q",
        ]))
        .unwrap();
        assert_eq!(teardown.mode, Mode::TeardownGroup);
    }

    #[test]
    fn group_setup_rejects_other_backends_and_run_flags() {
        for args in [
            vec!["--setup-group", "g"],
            vec!["--backend", "appcontainer", "--teardown-group", "g"],
            vec!["--backend", "user", "--setup-group", "g", "--group", "g"],
            vec!["--backend", "user", "--setup-group", "g", "--", "x"],
            vec!["--backend", "user", "--setup-group", "g", "--name", "n"],
            vec!["--backend", "user", "--setup-group", "g", "--net"],
            vec![
                "--backend",
                "user",
                "--setup-group",
                "g",
                "--teardown-group",
                "g",
            ],
            vec!["--backend", "user", "--setup-group", ""],
        ] {
            assert!(parse(&strings(&args)).is_err(), "{args:?}");
        }
    }

    #[test]
    fn group_pid_reads_only_run_group_names() {
        assert_eq!(group_pid("dxh-123"), Some(123));
        assert_eq!(group_pid("dxh-4294967295"), Some(u32::MAX));
        assert_eq!(group_pid("dxh-123-test"), None);
        assert_eq!(group_pid(&user_name("dx-hermetic-1-0")), None);
        assert_eq!(group_pid("dxh-0123456789abcdef"), None);
        assert_eq!(group_pid("dxh-"), None);
        assert_eq!(group_pid("dxh-+5"), None);
        assert_eq!(group_pid("dxh-99999999999"), None);
        assert_eq!(group_pid("dxh-4294967296"), None);
        assert_eq!(group_pid("Administrators"), None);
    }

    #[test]
    fn stale_groups_keeps_only_dead_pids_and_never_its_own() {
        let names = strings(&[
            "Administrators",
            "dxh-10",
            "dxh-11",
            "dxh-12",
            "dxh-12-test",
            "dxh-0123456789abcdef",
        ]);
        let alive = |pid| pid == 11;
        assert_eq!(stale_groups(&names, "dxh-12", alive), ["dxh-10"]);
        assert_eq!(stale_groups(&names, "dxh-99", alive), ["dxh-10", "dxh-12"]);
    }

    #[test]
    fn user_names_fit_the_account_limit_and_differ() {
        let one = user_name("dx-hermetic-1-0");
        assert_eq!(one.len(), 20);
        assert!(one.starts_with("dxh-"));
        assert_ne!(one, user_name("dx-hermetic-1-1"));
        assert_eq!(one, user_name("dx-hermetic-1-0"));
    }

    #[test]
    fn passwords_have_every_character_class() {
        let word = password(&[7; 24]);
        assert_eq!(word.len(), 24);
        assert!(word.chars().any(|c| c.is_ascii_uppercase()));
        assert!(word.chars().any(|c| c.is_ascii_lowercase()));
        assert!(word.chars().any(|c| c.is_ascii_digit()));
        assert!(word.chars().any(|c| !c.is_ascii_alphanumeric()));
    }

    #[test]
    fn network_is_denied_by_default() {
        assert!(!parse(&strings(&["--name", "n", "--", "x"])).unwrap().net);
    }

    #[test]
    fn rejects_bad_command_lines() {
        for args in [
            vec!["--", "x"],
            vec!["--name", "n"],
            vec!["--name", "n", "--"],
            vec!["--name", "bad name", "--", "x"],
            vec!["--name", "n", "--grant"],
            vec!["--name", "n", "--bogus", "--", "x"],
        ] {
            assert!(parse(&strings(&args)).is_err(), "{args:?}");
        }
    }

    #[test]
    fn logon_command_line_stops_at_1024_characters() {
        let fits = strings(&["bazel.exe", &"a".repeat(1014)]);
        assert_eq!(logon_command_line(&fits).unwrap().len(), 1024);
        let long = strings(&["bazel.exe", &"a".repeat(1015)]);
        let err = logon_command_line(&long).unwrap_err();
        assert!(err.contains("bazel.exe"), "{err}");
    }

    #[test]
    fn quotes_for_the_windows_command_line() {
        let line = command_line(&strings(&["a", "b c", "", "d\"e", "f g\\"]));
        assert_eq!(line, "a \"b c\" \"\" \"d\\\"e\" \"f g\\\\\"");
    }

    fn pairs(vars: &[(&str, &str)]) -> Vec<(String, String)> {
        vars.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    fn decode(block: &[u16]) -> Vec<String> {
        block
            .split(|c| *c == 0)
            .filter(|part| !part.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    #[test]
    fn parent_value_wins_over_default() {
        let (block, added) = environment(
            &pairs(&[("temp", "C:\\mine")]),
            &pairs(&[("TEMP", "C:\\other")]),
        );
        assert_eq!(decode(&block), ["temp=C:\\mine"]);
        assert!(added.is_empty());
    }

    #[test]
    fn missing_names_are_added_and_others_dropped() {
        let (block, added) = environment(
            &pairs(&[("FOO", "1")]),
            &pairs(&[("LOCALAPPDATA", "C:\\l"), ("ProgramFiles", "C:\\p")]),
        );
        assert_eq!(decode(&block), ["FOO=1", "LOCALAPPDATA=C:\\l"]);
        assert_eq!(added, ["LOCALAPPDATA"]);
    }

    #[test]
    fn sort_ignores_case_and_block_ends_in_two_nuls() {
        let (block, _) = environment(&pairs(&[("b", "1"), ("A", "2"), ("c", "3")]), &[]);
        assert_eq!(decode(&block), ["A=2", "b=1", "c=3"]);
        assert_eq!(block[block.len() - 2..], [0, 0]);
    }

    #[test]
    fn ancestors_run_from_the_drive_root_down() {
        let dirs = ancestors(&strings(&["D:\\a\\b c\\d"])).unwrap();
        assert_eq!(dirs, ["D:\\", "D:\\a", "D:\\a\\b c"]);
    }

    #[test]
    fn ancestors_of_two_grants_are_merged() {
        let dirs = ancestors(&strings(&["D:\\a\\b", "d:\\A\\c\\", "E:/x/"])).unwrap();
        assert_eq!(dirs, ["D:\\", "D:\\a", "E:\\"]);
    }

    #[test]
    fn ancestors_skip_granted_directories() {
        let dirs = ancestors(&strings(&["D:\\a", "D:\\a\\b\\c", "d:\\A\\B"])).unwrap();
        assert_eq!(dirs, ["D:\\"]);
        assert!(ancestors(&strings(&["D:\\"])).unwrap().is_empty());
    }

    #[test]
    fn ancestors_reject_unc_paths() {
        assert!(ancestors(&strings(&["\\\\server\\share\\x"])).is_err());
        assert!(ancestors(&strings(&["relative\\x"])).is_err());
        assert!(ancestors(&strings(&["D:x"])).is_err());
    }
}
