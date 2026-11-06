//! Command line of the AppContainer launcher: `--name N [--net] [--grant P]... -- cmd args`.
#![cfg_attr(test, allow(dead_code))]

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub name: String,
    pub net: bool,
    pub grants: Vec<String>,
    pub command: Vec<String>,
}

pub fn parse(args: &[String]) -> Result<Config, String> {
    let mut name = None;
    let mut net = false;
    let mut grants = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--name" => name = Some(iter.next().ok_or("--name needs a value")?.clone()),
            "--grant" => grants.push(iter.next().ok_or("--grant needs a path")?.clone()),
            "--net" => net = true,
            "--" => {
                let command: Vec<String> = iter.cloned().collect();
                if command.is_empty() {
                    return Err("no command after --".to_owned());
                }
                let name = name.ok_or("--name is required")?;
                if name.is_empty()
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
                {
                    return Err(format!("invalid container name {name:?}"));
                }
                return Ok(Config {
                    name,
                    net,
                    grants,
                    command,
                });
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Err("missing -- before the command".to_owned())
}

/// Joins arguments into one Windows command line, quoted for CommandLineToArgvW.
pub fn command_line(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
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
    fn quotes_for_the_windows_command_line() {
        let line = command_line(&strings(&["a", "b c", "", "d\"e", "f g\\"]));
        assert_eq!(line, "a \"b c\" \"\" \"d\\\"e\" \"f g\\\\\"");
    }
}
