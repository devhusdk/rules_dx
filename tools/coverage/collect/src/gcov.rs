use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FileCoverage {
    functions: BTreeMap<String, (u64, u64)>,
    lines: BTreeMap<u64, u64>,
}

pub type Report = BTreeMap<String, FileCoverage>;

pub fn object_key(path: &str) -> Option<String> {
    let start = if path.starts_with("bazel-out/")
        && path
            .get(1..)
            .is_some_and(|tail| !tail.contains("/bazel-out/"))
    {
        0
    } else {
        path.rfind("/bazel-out/")? + 1
    };
    let key = path.get(start..)?;
    Some(
        key.rsplit_once('.')
            .map_or(key, |(stem, _)| stem)
            .to_string(),
    )
}

pub fn gcno_by_key(manifest: &str) -> BTreeMap<String, String> {
    manifest
        .lines()
        .map(str::trim)
        .filter(|line| {
            Path::new(line)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gcno"))
        })
        .filter_map(|line| {
            let key = object_key(line)
                .or_else(|| line.rsplit_once('.').map(|(stem, _)| stem.to_string()))?;
            Some((key, line.to_string()))
        })
        .collect()
}

pub fn runfiles_key(key: &str, workspace: &str) -> Option<String> {
    let rest = key.strip_prefix("bazel-out/")?;
    let (_, rest) = rest.split_once('/')?;
    let rest = rest.strip_prefix("bin/")?;
    Some(match rest.strip_prefix("external/") {
        Some(external) => external.to_string(),
        None => format!("{workspace}/{rest}"),
    })
}

pub fn gcov_args(object_dir: &Path, gcda: &Path) -> Vec<String> {
    vec![
        "--json-format".to_string(),
        "--stdout".to_string(),
        "--object-directory".to_string(),
        object_dir.display().to_string(),
        gcda.display().to_string(),
    ]
}

pub fn gcov_problem(stderr: &str) -> Option<String> {
    const MARKERS: [&str; 7] = [
        "version mismatch",
        "prefer '",
        "stamp mismatch",
        "corrupt",
        "overflow",
        "cannot open notes file",
        "profile mismatch for",
    ];
    stderr
        .lines()
        .find(|line| {
            let line = line.to_lowercase();
            MARKERS.iter().any(|m| line.contains(m))
        })
        .map(|line| line.trim().to_string())
}

fn wanted(source: &str) -> bool {
    !Path::new(source).is_absolute() && !source.starts_with("external/")
}

pub fn add_gcov_json(report: &mut Report, json: &str) -> Result<(), String> {
    for doc in serde_json::Deserializer::from_str(json).into_iter::<Value>() {
        let doc = doc.map_err(|e| format!("bad gcov json: {e}"))?;
        let files = doc
            .get("files")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for file in files {
            let Some(source) = file["file"].as_str().filter(|s| wanted(s)) else {
                continue;
            };
            let entry = report.entry(source.to_string()).or_default();
            for function in file["functions"].as_array().into_iter().flatten() {
                let name = function["name"].as_str().unwrap_or_default();
                let start = function["start_line"].as_u64().unwrap_or(0);
                let count = function["execution_count"].as_u64().unwrap_or(0);
                let slot = entry
                    .functions
                    .entry(name.to_string())
                    .or_insert((start, 0));
                slot.1 += count;
            }
            for line in file["lines"].as_array().into_iter().flatten() {
                let number = line["line_number"].as_u64().unwrap_or(0);
                let count = line["count"].as_u64().unwrap_or(0);
                *entry.lines.entry(number).or_insert(0) += count;
            }
        }
    }
    Ok(())
}

pub fn render(report: &Report) -> String {
    let mut out = String::new();
    for (source, cov) in report {
        writeln!(out, "SF:{source}").unwrap_or_default();
        for (name, (start, _)) in &cov.functions {
            writeln!(out, "FN:{start},{name}").unwrap_or_default();
        }
        for (name, (_, count)) in &cov.functions {
            writeln!(out, "FNDA:{count},{name}").unwrap_or_default();
        }
        let hit_functions = cov.functions.values().filter(|f| f.1 > 0).count();
        writeln!(out, "FNF:{}\nFNH:{hit_functions}", cov.functions.len()).unwrap_or_default();
        for (line, count) in &cov.lines {
            writeln!(out, "DA:{line},{count}").unwrap_or_default();
        }
        let hit_lines = cov.lines.values().filter(|c| **c > 0).count();
        writeln!(out, "LF:{}\nLH:{hit_lines}\nend_of_record", cov.lines.len()).unwrap_or_default();
    }
    out
}

pub fn gcda_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            gcda_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "gcda") {
            found.push(path);
        }
    }
    found.sort();
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"format_version":"2","files":[
      {"file":"cc/store.c","functions":[{"name":"add","start_line":3,"execution_count":2}],
       "lines":[{"line_number":3,"count":2},{"line_number":4,"count":0}]},
      {"file":"/usr/include/x.h","functions":[],"lines":[{"line_number":1,"count":1}]},
      {"file":"external/dep/y.c","functions":[],"lines":[{"line_number":1,"count":1}]}]}"#;

    #[test]
    fn keys_start_at_bazel_out() {
        assert_eq!(
            object_key("bazel-out/k8/bin/a/_objs/t/a.gcno").as_deref(),
            Some("bazel-out/k8/bin/a/_objs/t/a")
        );
        assert_eq!(
            object_key("/cov/x/y/bazel-out/k8/bin/a.gcda").as_deref(),
            Some("bazel-out/k8/bin/a")
        );
        assert_eq!(
            object_key("/root/bazel-out/k8/testlogs/cov/bazel-out/k8/bin/a.pic.gcda").as_deref(),
            Some("bazel-out/k8/bin/a.pic")
        );
        assert_eq!(object_key("elsewhere/a.gcda"), None);
    }

    #[test]
    fn runfiles_keys_drop_the_configuration() {
        assert_eq!(
            runfiles_key("bazel-out/k8-ST-1/bin/cc/_objs/t/a.pic", "_main").as_deref(),
            Some("_main/cc/_objs/t/a.pic")
        );
        assert_eq!(
            runfiles_key("bazel-out/k8/bin/external/+dep/_objs/t/a", "_main").as_deref(),
            Some("+dep/_objs/t/a")
        );
        assert_eq!(runfiles_key("bazel-out/k8/testlogs/a", "_main"), None);
        let map = gcno_by_key("_main/cc/_objs/t/a.pic.gcno\n");
        assert_eq!(map["_main/cc/_objs/t/a.pic"], "_main/cc/_objs/t/a.pic.gcno");
    }

    #[test]
    fn manifest_lists_only_gcno() {
        let map = gcno_by_key("a/b.c\nbazel-out/k8/bin/a.gcno\nbazel-out/k8/bin/a.o\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map["bazel-out/k8/bin/a"], "bazel-out/k8/bin/a.gcno");
    }

    #[test]
    fn sums_and_renders_lcov() {
        let mut report = Report::new();
        add_gcov_json(&mut report, SAMPLE).unwrap();
        add_gcov_json(&mut report, SAMPLE).unwrap();
        assert_eq!(
            render(&report),
            "SF:cc/store.c\nFN:3,add\nFNDA:4,add\nFNF:1\nFNH:1\nDA:3,4\nDA:4,0\nLF:2\nLH:1\nend_of_record\n"
        );
    }

    #[test]
    fn rejects_bad_json() {
        assert!(add_gcov_json(&mut Report::new(), "{").is_err());
    }

    #[test]
    fn flags_gcov_problems() {
        for line in [
            "a.gcda:version 'B52*', prefer 'B62*'",
            "a.gcda:stamp mismatch with notes file",
            "a.gcda:corrupted profile",
            "a.gcno:cannot open notes file",
            "a.gcda:overflowed arc counter",
            "a.gcda:profile mismatch for 'main'",
        ] {
            let found = gcov_problem(&format!("ok\n{line}\n"));
            assert_eq!(found.as_deref(), Some(line));
        }
        assert_eq!(gcov_problem("a.gcda:profiling:ok\n"), None);
    }

    #[test]
    fn builds_gcov_args() {
        let args = gcov_args(Path::new("/d"), Path::new("/d/a.gcda"));
        assert_eq!(args[2..], ["--object-directory", "/d", "/d/a.gcda"]);
    }
}
