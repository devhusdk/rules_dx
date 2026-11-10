//! Runs PHPUnit with PCOV loaded and turns the Clover report it writes into the LCOV file Bazel collects.
//!
//! Arguments: `<tree> <pcov> <php> [<php flag>...] -- <phpunit> [<phpunit argument>...]`.

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
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

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

fn convert(clover: &Path, output: &Path, marker: &str) -> Result<(), String> {
    let xml = fs::read_to_string(clover).map_err(|err| format!("read {}: {err}", clover.display()))?;
    let lcov = dx_lcov::clover_to_lcov(&xml).map_err(|err| format!("{}: {err}", clover.display()))?;
    fs::write(output, rebase(&lcov, marker)).map_err(|err| format!("write {}: {err}", output.display()))
}

/// What the launcher asks for: the autoload tree, PCOV, PHP with its flags and PHPUnit with its arguments.
#[derive(Debug, PartialEq, Eq)]
struct Request {
    tree: PathBuf,
    pcov: OsString,
    php: OsString,
    flags: Vec<OsString>,
    phpunit: OsString,
    rest: Vec<OsString>,
}

fn request(args: &[OsString]) -> Option<Request> {
    let [tree, pcov, php, tail @ ..] = args else {
        return None;
    };
    let split = tail.iter().position(|arg| arg == "--")?;
    let (phpunit, rest) = tail.get(split + 1..)?.split_first()?;
    Some(Request {
        tree: PathBuf::from(tree),
        pcov: pcov.clone(),
        php: php.clone(),
        flags: tail.get(..split)?.to_vec(),
        phpunit: phpunit.clone(),
        rest: rest.to_vec(),
    })
}

fn joined(prefix: &str, value: &std::ffi::OsStr) -> OsString {
    let mut out = OsString::from(prefix);
    out.push(value);
    out
}

/// The arguments PHP gets: PCOV on `tree`, a Clover report at `clover`.
fn php_args(request: &Request, tree: &Path, clover: &Path) -> Vec<OsString> {
    let mut args = request.flags.clone();
    args.extend([
        "-d".into(),
        joined("extension=", &request.pcov),
        "-d".into(),
        "pcov.enabled=1".into(),
        "-d".into(),
        joined("pcov.directory=", tree.as_os_str()),
        request.phpunit.clone(),
        "--coverage-filter".into(),
        tree.into(),
        "--coverage-clover".into(),
        clover.into(),
    ]);
    args.extend(request.rest.iter().cloned());
    args
}

/// The marker that ends the tree in every source path of the report.
fn marker(tree: &Path) -> String {
    let name = tree.file_name().unwrap_or_default().to_string_lossy();
    format!("/{name}/")
}

fn run(request: &Request) -> Result<i32, String> {
    let tree = request
        .tree
        .canonicalize()
        .map_err(|err| format!("{}: {err}", request.tree.display()))?;
    let scratch = env::var_os("TEST_TMPDIR").map_or_else(env::temp_dir, PathBuf::from);
    let clover = scratch.join(format!("clover.{}.xml", process::id()));
    let status = Command::new(&request.php)
        .args(php_args(request, &tree, &clover))
        .status()
        .map_err(|err| format!("cannot run {}: {err}", Path::new(&request.php).display()))?;
    let written = fs::metadata(&clover).is_ok_and(|meta| meta.len() > 0);
    if let Some(output) = env::var_os("COVERAGE_OUTPUT_FILE").filter(|value| written && !value.is_empty()) {
        convert(&clover, Path::new(&output), &marker(&request.tree))?;
    }
    Ok(status.code().unwrap_or(1))
}

fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let Some(request) = request(&args) else {
        eprintln!("usage: coverage_run <tree> <pcov> <php> [<php flag>...] -- <phpunit> [<argument>...]");
        process::exit(2);
    };
    match run(&request) {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("coverage_run: {err}");
            process::exit(1);
        }
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
        let dir = env::temp_dir().join(format!("coverage_run_{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        let clover = dir.join("clover.xml");
        let out = dir.join("out.lcov");
        fs::write(
            &clover,
            "<coverage><project><file name=\"/o/t.php/app/A.php\"><line num=\"3\" type=\"stmt\" count=\"1\"/></file></project></coverage>",
        )
        .unwrap();
        convert(&clover, &out, "/t.php/").unwrap();
        let text = fs::read_to_string(&out).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(text, "SF:app/A.php\nDA:3,1\nLF:1\nLH:1\nend_of_record\n");
    }

    #[test]
    fn a_missing_report_is_an_error() {
        assert!(convert(Path::new("no/such/clover.xml"), Path::new("out.lcov"), "/t.php/").is_err());
    }

    fn words(text: &str) -> Vec<OsString> {
        text.split(' ').map(OsString::from).collect()
    }

    #[test]
    fn splits_the_php_flags_from_the_phpunit_arguments() {
        let parsed = request(&words("o/t.php pcov.so bin/php -n -d a=b -- phpunit.phar --bootstrap x T.php --filter y")).unwrap();
        assert_eq!(parsed.tree, PathBuf::from("o/t.php"));
        assert_eq!(parsed.flags, words("-n -d a=b"));
        assert_eq!(parsed.phpunit, "phpunit.phar");
        assert_eq!(parsed.rest, words("--bootstrap x T.php --filter y"));
        assert_eq!(marker(&parsed.tree), "/t.php/");
    }

    #[test]
    fn a_request_without_phpunit_is_rejected() {
        assert_eq!(request(&words("o/t.php pcov.so bin/php -n")), None);
        assert_eq!(request(&words("o/t.php pcov.so bin/php -n --")), None);
        assert_eq!(request(&words("o/t.php pcov.so")), None);
    }

    #[test]
    fn php_loads_pcov_on_the_tree_and_phpunit_writes_clover() {
        let parsed = request(&words("o/t.php pcov.so bin/php -n -- phpunit.phar --bootstrap x T.php")).unwrap();
        assert_eq!(
            php_args(&parsed, Path::new("/real/t.php"), Path::new("/scratch/c.xml")),
            words("-n -d extension=pcov.so -d pcov.enabled=1 -d pcov.directory=/real/t.php phpunit.phar --coverage-filter /real/t.php --coverage-clover /scratch/c.xml --bootstrap x T.php")
        );
    }
}
