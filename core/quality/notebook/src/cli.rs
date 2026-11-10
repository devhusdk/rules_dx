//! Command line front end of `quality_notebook`.

use crate::{output_findings, strip_outputs};

const USAGE: &str = "usage: quality_notebook outputs [--fix] FILE...";

/// Runs the CLI. Exit codes: 0 clean (or fixed), 1 findings, 2 usage or
/// input errors.
pub fn run_cli(
    args: &[String],
    read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>,
    write: &mut dyn FnMut(&str, &[u8]) -> std::io::Result<()>,
    out: &mut dyn FnMut(&str),
    err: &mut dyn FnMut(&str),
) -> i32 {
    let Some((command, rest)) = args.split_first() else {
        err(USAGE);
        return 2;
    };
    if command != "outputs" {
        err(&format!("unknown subcommand {command:?}; {USAGE}"));
        return 2;
    }
    let mut fix = false;
    let mut files = Vec::new();
    for arg in rest {
        match arg.as_str() {
            "--fix" => fix = true,
            flag if flag.starts_with("--") => {
                err(&format!("unknown flag {flag:?}; {USAGE}"));
                return 2;
            }
            _ => files.push(arg.as_str()),
        }
    }
    if files.is_empty() {
        err(USAGE);
        return 2;
    }
    let mut dirty = false;
    for file in files {
        let text = match read(file).map(String::from_utf8) {
            Ok(Ok(text)) => text,
            Ok(Err(why)) => {
                err(&format!("{file}: not UTF-8: {why}"));
                return 2;
            }
            Err(why) => {
                err(&format!("{file}: {why}"));
                return 2;
            }
        };
        if fix {
            match strip_outputs(&text) {
                Ok(fixed) if fixed == text => {}
                Ok(fixed) => {
                    if let Err(why) = write(file, fixed.as_bytes()) {
                        err(&format!("{file}: {why}"));
                        return 2;
                    }
                }
                Err(why) => {
                    err(&format!("{file}: {why}"));
                    return 2;
                }
            }
            continue;
        }
        match output_findings(&text) {
            Ok(found) => {
                dirty |= !found.is_empty();
                for item in found {
                    out(&serde_json::json!({
                        "path": file,
                        "cell": item.cell,
                        "line": item.line,
                        "column": item.column,
                        "rule": item.rule,
                        "message": item.message,
                    })
                    .to_string());
                }
            }
            Err(why) => {
                err(&format!("{file}: {why}"));
                return 2;
            }
        }
    }
    i32::from(dirty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    const DIRTY: &str = "{\"cells\": [{\"cell_type\": \"code\", \"execution_count\": 1, \"metadata\": {}, \"outputs\": [{\"output_type\": \"stream\"}], \"source\": []}], \"metadata\": {}, \"nbformat\": 4}";

    fn run(
        args: &[&str],
        files: &[(&str, &str)],
    ) -> (i32, Vec<String>, Vec<String>, HashMap<String, String>) {
        let store: RefCell<HashMap<String, String>> = RefCell::new(
            files
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        );
        let mut out = Vec::new();
        let mut err = Vec::new();
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let code = run_cli(
            &args,
            &|path| {
                store
                    .borrow()
                    .get(path)
                    .map(|s| s.clone().into_bytes())
                    .ok_or_else(|| std::io::Error::other("missing"))
            },
            &mut |path, bytes| {
                store
                    .borrow_mut()
                    .insert(path.to_owned(), String::from_utf8_lossy(bytes).into_owned());
                Ok(())
            },
            &mut |line| out.push(line.to_owned()),
            &mut |line| err.push(line.to_owned()),
        );
        (code, out, err, store.into_inner())
    }

    #[test]
    fn findings_exit_one_and_print_json_lines() {
        let (code, out, _, _) = run(&["outputs", "a.ipynb"], &[("a.ipynb", DIRTY)]);
        assert_eq!(code, 1);
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("\"rule\":\"execution-count\""));
    }

    #[test]
    fn clean_exits_zero_and_fix_rewrites() {
        let (code, _, _, files) = run(&["outputs", "--fix", "a.ipynb"], &[("a.ipynb", DIRTY)]);
        assert_eq!(code, 0);
        let (code, out, _, _) = run(&["outputs", "a.ipynb"], &[("a.ipynb", &files["a.ipynb"])]);
        assert_eq!((code, out.len()), (0, 0));
    }

    #[test]
    fn usage_and_input_errors_exit_two() {
        for args in [
            vec![],
            vec!["other"],
            vec!["outputs"],
            vec!["outputs", "--bogus", "a"],
            vec!["outputs", "missing.ipynb"],
            vec!["outputs", "--fix", "bad.ipynb"],
            vec!["outputs", "bad.ipynb"],
        ] {
            let (code, _, err, _) = run(&args, &[("bad.ipynb", "{")]);
            assert_eq!(code, 2, "{args:?}");
            assert!(!err.is_empty());
        }
    }
}
