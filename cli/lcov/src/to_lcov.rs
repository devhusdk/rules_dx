//! Converts a Clover or Cobertura XML coverage report to LCOV.

#![deny(
    clippy::map_err_ignore,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unused_result_ok
)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use dx_error::DxError;

const USAGE: &str = "usage: to_lcov --from clover|cobertura [--sources <file>] <in> <out>";

#[derive(Debug, PartialEq, Eq)]
struct Args {
    from: String,
    sources: Option<PathBuf>,
    input: PathBuf,
    output: PathBuf,
}

fn parse_args(args: &[OsString]) -> Result<Args, DxError> {
    let mut from = None;
    let mut sources = None;
    let mut paths = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--from") => from = iter.next().map(|v| v.to_string_lossy().into_owned()),
            Some("--sources") => sources = iter.next().map(PathBuf::from),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let from = from.ok_or_else(|| DxError::usage(USAGE))?;
    let [input, output] = <[PathBuf; 2]>::try_from(paths).map_err(|paths: Vec<PathBuf>| {
        DxError::usage(format!(
            "{USAGE}: want <in> <out>, got {} path arguments",
            paths.len()
        ))
    })?;
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

fn run(args: &[OsString]) -> Result<(), DxError> {
    let args = parse_args(args)?;
    let xml = fs::read_to_string(&args.input)
        .map_err(|e| DxError::infra(format!("cannot read {}: {e}", args.input.display())))?;
    let mut lcov = convert(&args.from, &xml)?;
    if let Some(path) = &args.sources {
        let list = fs::read_to_string(path)
            .map_err(|e| DxError::infra(format!("cannot read {}: {e}", path.display())))?;
        let sources: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
        lcov = dx_lcov::keep_sources(&lcov, &sources);
    }
    dx_atomic_fs::write_atomic(&args.output, lcov.as_bytes())
        .map_err(|e| DxError::infra(format!("cannot write {}: {e}", args.output.display())))
}

// LCOV_EXCL_START - reason: process entry point that exits, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("dx: {error}"));
            error.into()
        }
    }
}
// LCOV_EXCL_STOP - reason: end process entry point, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn strings(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
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
                sources: Some(PathBuf::from("list")),
                input: PathBuf::from("in"),
                output: PathBuf::from("out"),
            })
        );
    }

    #[test]
    fn rejects_missing_from_or_paths() {
        assert!(parse_args(&strings(&["in", "out"])).is_err());
        let error = parse_args(&strings(&["--from", "clover", "in"])).expect_err("one path");
        assert!(error.message().contains("got 1 path"), "{error}");
    }

    #[test]
    fn rejects_unknown_formats() {
        assert!(convert("jacoco", "<report/>").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reads_and_writes_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let dir = env::temp_dir().join(format!("to_lcov_raw_{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let mut name = OsString::from_vec(b"in-\xff.xml".to_vec());
        let input = dir.join(&name);
        fs::write(
            &input,
            "<coverage><project><file name=\"/w/a.php\"><line num=\"1\" type=\"stmt\" count=\"1\"/></file></project></coverage>",
        )
        .expect("write input");
        name = OsString::from_vec(b"out-\xfe.dat".to_vec());
        let output = dir.join(&name);
        let args = [
            OsString::from("--from"),
            OsString::from("clover"),
            input.into_os_string(),
            output.clone().into_os_string(),
        ];
        run(&args).expect("converts");
        assert!(output.exists());
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
        args.push(list.into_os_string());
        args.push(input.into_os_string());
        args.push(output.clone().into_os_string());
        run(&args).expect("converts");
        let lcov = fs::read_to_string(&output).expect("read output");
        assert_eq!(lcov, "SF:src/a.zig\nDA:1,2\nLF:1\nLH:1\nend_of_record\n");
    }

    #[test]
    fn replaces_an_existing_output_whole() {
        let dir = env::temp_dir().join(format!("to_lcov_replace_{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let input = dir.join("clover.xml");
        let output = dir.join("coverage.dat");
        fs::write(
            &input,
            "<coverage><project><file name=\"/w/a.php\"><line num=\"1\" type=\"stmt\" count=\"1\"/></file></project></coverage>",
        )
        .expect("write input");
        fs::write(&output, "SF:stale.php\n".repeat(64)).expect("seed output");
        let args = [
            OsString::from("--from"),
            OsString::from("clover"),
            input.into_os_string(),
            output.clone().into_os_string(),
        ];
        run(&args).expect("converts");
        let lcov = fs::read_to_string(&output).expect("read output");
        assert_eq!(lcov, "SF:/w/a.php\nDA:1,1\nLF:1\nLH:1\nend_of_record\n");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .expect("list dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
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
        // write_atomic creates missing parents, so a file in the parent's place is what fails.
        let output = dir.join("clover.xml").join("out").display().to_string();

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
