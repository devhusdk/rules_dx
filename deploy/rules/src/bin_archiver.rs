#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

fn split_member(pair: &str) -> Option<(String, PathBuf)> {
    pair.split_once('=')
        .map(|(name, path)| (name.to_owned(), PathBuf::from(path)))
}

fn members_of(argv: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let mut members = Vec::new();
    for pair in argv.iter().skip(3) {
        match split_member(pair) {
            Some(member) => members.push(member),
            None => {
                return Err(format!("archiver: member {pair:?} must be <name=path>"));
            }
        }
    }
    Ok(members)
}

fn single_job(argv: &[String]) -> Option<(PathBuf, PathBuf)> {
    if argv.len() != 3 {
        return None;
    }
    Some((PathBuf::from(&argv[1]), PathBuf::from(&argv[2])))
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    if argv.len() >= 2 && argv[1] == "--members" {
        if argv.len() < 4 {
            return dx_deploy_tools::bin_usage("archiver", "--members <out> <name=path>...");
        }
        let dst = PathBuf::from(&argv[2]);
        let members = match members_of(argv) {
            Ok(members) => members,
            Err(diagnostic) => {
                eprintln!("{diagnostic}");
                return 1;
            }
        };
        let borrowed: Vec<(&str, &std::path::Path)> = members
            .iter()
            .map(|(name, path)| (name.as_str(), path.as_path()))
            .collect();
        return match dx_deploy_tools::archive_members(&borrowed, &dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", &dst, error),
        };
    }
    let (src, dst) = match single_job(argv) {
        Some(job) => job,
        None => return dx_deploy_tools::bin_usage("archiver", "<src> <out>"),
    };
    match dx_deploy_tools::archive_file(&src, &dst) {
        Ok(()) => 0,
        Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", &src, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn member_split_keeps_name_and_path_verbatim() {
        assert_eq!(
            split_member("tool=bin/tool"),
            Some(("tool".to_owned(), PathBuf::from("bin/tool")))
        );
        assert_eq!(split_member("no-equals-here"), None);
    }

    #[test]
    fn members_of_collects_every_pair_after_the_output() {
        assert_eq!(
            members_of(&argv_of(&["prog", "--members", "out.tar", "a=x", "b=y"])),
            Ok(vec![
                ("a".to_owned(), PathBuf::from("x")),
                ("b".to_owned(), PathBuf::from("y")),
            ])
        );
    }

    #[test]
    fn members_of_names_the_first_malformed_pair() {
        assert_eq!(
            members_of(&argv_of(&["prog", "--members", "out.tar", "a=x", "oops"])),
            Err("archiver: member \"oops\" must be <name=path>".to_owned())
        );
    }

    #[test]
    fn single_job_binds_positionals_and_rejects_other_arity() {
        assert_eq!(
            single_job(&argv_of(&["prog", "a.dir", "out.tar"])),
            Some((PathBuf::from("a.dir"), PathBuf::from("out.tar")))
        );
        assert_eq!(single_job(&argv_of(&["prog", "a"])), None);
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["archiver"])), 1);
        assert_eq!(run(&argv_of(&["prog", "--members"])), 1);
    }
}
