//! Turns the Clover report PHPUnit writes under coverage into the LCOV file Bazel collects.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::env;
use std::fs;
use std::process;

const DROPPED: [&str; 2] = ["vendor/", "external/"];

/// Keeps the records of sources below `marker` and rewrites their paths relative to it.
fn rebase(lcov: &str, marker: &str) -> String {
    let mut out = String::new();
    let mut keep = false;
    for line in lcov.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let rest = path
                .split_once(marker)
                .map(|(_, rest)| rest)
                .filter(|rest| !DROPPED.iter().any(|dir| rest.starts_with(dir)));
            keep = rest.is_some();
            if let Some(rest) = rest {
                out.push_str(&format!("SF:{rest}\n"));
            }
        } else if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn run(clover: &str, output: &str, marker: &str) -> Result<(), String> {
    let xml = fs::read_to_string(clover).map_err(|err| format!("read {clover}: {err}"))?;
    let lcov = dx_lcov::clover_to_lcov(&xml).map_err(|err| format!("{clover}: {err}"))?;
    fs::write(output, rebase(&lcov, marker)).map_err(|err| format!("write {output}: {err}"))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [clover, output, marker] = args.as_slice() else {
        eprintln!("usage: clover_to_lcov <clover.xml> <out.lcov> <marker>");
        process::exit(2);
    };
    if let Err(err) = run(clover, output, marker) {
        eprintln!("clover_to_lcov: {err}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCOV: &str = "SF:/o/x/t.php/app/A.php\nDA:1,2\nLF:1\nLH:1\nend_of_record\nSF:/o/x/t.php/vendor/autoload.php\nDA:1,1\nend_of_record\nSF:/o/x/t.php/external/dep+/B.php\nDA:1,1\nend_of_record\nSF:/usr/phpunit.phar\nDA:1,1\nend_of_record\n";

    #[test]
    fn keeps_sources_below_the_tree() {
        assert_eq!(
            rebase(LCOV, "/t.php/"),
            "SF:app/A.php\nDA:1,2\nLF:1\nLH:1\nend_of_record\n"
        );
    }

    #[test]
    fn an_unknown_tree_keeps_nothing() {
        assert_eq!(rebase(LCOV, "/other.php/"), "");
    }

    #[test]
    fn converts_a_clover_report_on_disk() {
        let dir = env::temp_dir().join(format!("clover_to_lcov_{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        let clover = dir.join("clover.xml");
        let out = dir.join("out.lcov");
        fs::write(
            &clover,
            "<coverage><project><file name=\"/o/t.php/app/A.php\"><line num=\"3\" type=\"stmt\" count=\"1\"/></file></project></coverage>",
        )
        .unwrap();
        run(clover.to_str().unwrap(), out.to_str().unwrap(), "/t.php/").unwrap();
        let text = fs::read_to_string(&out).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(text, "SF:app/A.php\nDA:3,1\nLF:1\nLH:1\nend_of_record\n");
    }

    #[test]
    fn a_missing_report_is_an_error() {
        assert!(run("/no/such/clover.xml", "/dev/null", "/t.php/").is_err());
    }
}
