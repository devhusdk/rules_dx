//! Converts a Clover or Cobertura XML coverage report to LCOV.

use std::env;
use std::fs;
use std::process;

const USAGE: &str = "usage: to_lcov --from clover|cobertura [--sources <file>] <in> <out>";

#[derive(Debug, PartialEq, Eq)]
struct Args {
    from: String,
    sources: Option<String>,
    input: String,
    output: String,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
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
    let from = from.ok_or(USAGE)?;
    let [input, output] = <[String; 2]>::try_from(paths).map_err(|_| USAGE)?;
    Ok(Args {
        from,
        sources,
        input,
        output,
    })
}

fn convert(from: &str, xml: &str) -> Result<String, String> {
    let lcov = match from {
        "clover" => dx_lcov::clover_to_lcov(xml),
        "cobertura" => dx_lcov::cobertura_to_lcov(xml),
        _ => return Err(format!("unknown --from {from}; want clover or cobertura")),
    };
    lcov.map_err(|e| e.to_string())
}

/// Keeps the records whose path is a listed source or ends in `/<source>`, renamed to it.
fn keep_sources(lcov: &str, sources: &[&str]) -> String {
    let mut out = String::new();
    let mut keep = false;
    for line in lcov.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let found = sources
                .iter()
                .find(|src| path == **src || path.ends_with(&format!("/{src}")));
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

fn run(args: &[String]) -> Result<(), String> {
    let args = parse_args(args)?;
    let xml =
        fs::read_to_string(&args.input).map_err(|e| format!("cannot read {}: {e}", args.input))?;
    let mut lcov = convert(&args.from, &xml)?;
    if let Some(path) = &args.sources {
        let list = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let sources: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
        lcov = keep_sources(&lcov, &sources);
    }
    fs::write(&args.output, lcov).map_err(|e| format!("cannot write {}: {e}", args.output))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Err(message) = run(&args) {
        eprintln!("dx: {message}");
        process::exit(1);
    }
}

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
    fn converts_cobertura_end_to_end() {
        let dir = env::temp_dir().join(format!("to_lcov_{}", process::id()));
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
}
