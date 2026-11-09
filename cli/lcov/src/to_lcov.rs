//! Converts a Clover or Cobertura XML coverage report to LCOV.

use std::env;
use std::fs;
use std::process::ExitCode;

use dx_error::DxError;

const USAGE: &str = "usage: to_lcov --from clover|cobertura [--sources <file>] <in> <out>";

#[derive(Debug, PartialEq, Eq)]
struct Args {
    from: String,
    sources: Option<String>,
    input: String,
    output: String,
}

fn parse_args(args: &[String]) -> Result<Args, DxError> {
    let mut from = None;
    let mut sources = None;
    let mut paths = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--from" => from = iter.next().cloned(),
            "--sources" => sources = iter.next().cloned(),
            _ => paths.push(arg.clone()),
        }
    }
    let from = from.ok_or_else(|| DxError::usage(USAGE))?;
    let [input, output] = <[String; 2]>::try_from(paths).map_err(|_| DxError::usage(USAGE))?;
    Ok(Args {
        from,
        sources,
        input,
        output,
    })
}

fn convert(from: &str, xml: &str) -> Result<String, DxError> {
    let lcov = match from {
        "clover" => dx_lcov::clover_to_lcov(xml),
        "cobertura" => dx_lcov::cobertura_to_lcov(xml),
        _ => {
            return Err(DxError::usage(format!(
                "unknown --from {from}; want clover or cobertura"
            )));
        }
    };
    lcov.map_err(|e| DxError::failure(e.to_string()))
}

/// Keeps the records whose path is a listed source or ends in `/<source>`, renamed to the longest such source.
fn keep_sources(lcov: &str, sources: &[&str]) -> String {
    let mut out = String::new();
    let mut keep = false;
    for line in lcov.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let found = sources.iter().find(|src| path == **src).or_else(|| {
                sources
                    .iter()
                    .filter(|src| path.ends_with(&format!("/{src}")))
                    .max_by_key(|src| src.len())
            });
            keep = found.is_some();
            if let Some(src) = found {
                out.push_str(&format!("SF:{src}\n"));
            }
            continue;
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn run(args: &[String]) -> Result<(), DxError> {
    let args = parse_args(args)?;
    let xml = fs::read_to_string(&args.input)
        .map_err(|e| DxError::infra(format!("cannot read {}: {e}", args.input)))?;
    let mut lcov = convert(&args.from, &xml)?;
    if let Some(path) = &args.sources {
        let list = fs::read_to_string(path)
            .map_err(|e| DxError::infra(format!("cannot read {path}: {e}")))?;
        let sources: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
        lcov = keep_sources(&lcov, &sources);
    }
    fs::write(&args.output, lcov)
        .map_err(|e| DxError::infra(format!("cannot write {}: {e}", args.output)))
}

// LCOV_EXCL_START - reason: process entry point that exits, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dx: {error}");
            error.into()
        }
    }
}
// LCOV_EXCL_STOP - reason: end process entry point, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn parses_from_sources_and_paths() {
        let args = parse_args(&strings(&[
            "--from",
            "cobertura",
            "--sources",
            "list",
            "in",
            "out",
        ]));
        assert_eq!(
            args,
            Ok(Args {
                from: "cobertura".into(),
                sources: Some("list".into()),
                input: "in".into(),
                output: "out".into(),
            })
        );
    }

    #[test]
    fn rejects_missing_from_or_paths() {
        assert!(parse_args(&strings(&["in", "out"])).is_err());
        assert!(parse_args(&strings(&["--from", "clover", "in"])).is_err());
    }

    #[test]
    fn rejects_unknown_formats() {
        assert!(convert("jacoco", "<report/>").is_err());
    }

    #[test]
    fn keeps_listed_sources_under_their_listed_path() {
        let lcov = "SF:/x/execroot/_main/src/a.zig\nDA:1,1\nend_of_record\n\
                    SF:/x/lib/std/mem.zig\nDA:2,0\nend_of_record\n\
                    SF:src/b.zig\nDA:3,1\nend_of_record\n";
        assert_eq!(
            keep_sources(lcov, &["src/a.zig", "src/b.zig"]),
            "SF:src/a.zig\nDA:1,1\nend_of_record\nSF:src/b.zig\nDA:3,1\nend_of_record\n"
        );
    }

    #[test]
    fn keeps_exact_match_over_shorter_suffix() {
        let lcov = "SF:tools/main.zig\nDA:1,1\nend_of_record\nSF:main.zig\nDA:2,0\nend_of_record\n";
        assert_eq!(keep_sources(lcov, &["main.zig", "tools/main.zig"]), lcov);
    }

    #[test]
    fn prefers_longest_suffix_in_any_order() {
        let lcov = "SF:/x/_main/tools/main.zig\nDA:1,1\nend_of_record\n\
                    SF:/x/_main/main.zig\nDA:2,0\nend_of_record\n";
        let want = "SF:tools/main.zig\nDA:1,1\nend_of_record\nSF:main.zig\nDA:2,0\nend_of_record\n";
        assert_eq!(keep_sources(lcov, &["main.zig", "tools/main.zig"]), want);
        assert_eq!(keep_sources(lcov, &["tools/main.zig", "main.zig"]), want);
    }

    #[test]
    fn converts_cobertura_end_to_end() {
        let dir = env::temp_dir().join(format!("to_lcov_{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let input = dir.join("cobertura.xml");
        let output = dir.join("coverage.dat");
        let list = dir.join("sources");
        fs::write(&list, "src/a.zig\n").expect("write sources");
        fs::write(
            &input,
            r#"<coverage><sources><source>/w/</source></sources><classes>
<class name="a" filename="src/a.zig"><lines><line number="1" hits="2"/></lines></class>
<class name="s" filename="/zig/lib/std/mem.zig"><lines><line number="5" hits="1"/></lines></class>
</classes></coverage>"#,
        )
        .expect("write input");
        let mut args = strings(&["--from", "cobertura", "--sources"]);
        args.push(list.display().to_string());
        args.push(input.display().to_string());
        args.push(output.display().to_string());
        run(&args).expect("converts");
        let lcov = fs::read_to_string(&output).expect("read output");
        assert_eq!(lcov, "SF:src/a.zig\nDA:1,2\nLF:1\nLH:1\nend_of_record\n");
    }

    #[test]
    fn reports_unreadable_input_sources_and_unwritable_output() {
        let dir = env::temp_dir().join(format!("to_lcov_err_{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let input = dir.join("clover.xml");
        fs::write(
            &input,
            "<coverage><project><file name=\"/w/a.php\"><line num=\"1\" type=\"stmt\" count=\"1\"/></file></project></coverage>",
        ).expect("write input");
        let missing = dir.join("missing").display().to_string();
        let input = input.display().to_string();
        let output = dir.join("no_dir").join("out").display().to_string();

        let read = run(&strings(&["--from", "clover", &missing, &output]));
        assert!(
            read.expect_err("input")
                .message()
                .starts_with("cannot read")
        );

        let sources = run(&strings(&[
            "--from",
            "clover",
            "--sources",
            &missing,
            &input,
            &output,
        ]));
        assert!(
            sources
                .expect_err("sources")
                .message()
                .starts_with("cannot read")
        );

        let write = run(&strings(&["--from", "clover", &input, &output]));
        let error = write.expect_err("output");
        let message = error.message();
        assert!(message.starts_with("cannot write"), "{message}");
    }
}
