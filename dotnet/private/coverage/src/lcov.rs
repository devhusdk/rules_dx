//! Rewrites LCOV source paths to workspace-relative form; language-neutral.

fn workspace_relative(path: &str, prefixes: &[String]) -> Option<String> {
    let mut path = path.replace('\\', "/");
    for prefix in prefixes {
        let prefix = prefix.replace('\\', "/");
        let prefix = prefix.trim_end_matches('/');
        if prefix.is_empty() {
            continue;
        }
        if let Some(rest) = path.strip_prefix(&format!("{prefix}/")) {
            path = rest.to_string();
            break;
        }
    }
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.to_string();
    }
    let absolute = path.starts_with('/') || path.as_bytes().get(1) == Some(&b':');
    if path.is_empty() || absolute || path.starts_with("../") || path.starts_with("external/") {
        return None;
    }
    Some(path)
}

/// Returns the report with workspace-relative SF: paths, dropping foreign records, and the kept record count.
pub fn normalize(report: &str, prefixes: &[String]) -> (String, usize) {
    let mut out = String::new();
    let mut kept = 0;
    let mut skipping = false;
    for line in report.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(path) = line.strip_prefix("SF:") {
            match workspace_relative(path, prefixes) {
                Some(path) => {
                    skipping = false;
                    kept += 1;
                    out.push_str("SF:");
                    out.push_str(&path);
                    out.push('\n');
                }
                None => skipping = true,
            }
            continue;
        }
        if skipping {
            if line == "end_of_record" {
                skipping = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_dot_slash() {
        let (out, kept) = normalize("SF:./pkg/Greeter.cs\nDA:7,3\nend_of_record\n", &[]);
        assert_eq!(out, "SF:pkg/Greeter.cs\nDA:7,3\nend_of_record\n");
        assert_eq!(kept, 1);
    }

    #[test]
    fn strips_mirror_prefix() {
        let prefixes = vec!["/tmp/t/dx_coverlet/runfiles/_main".to_string()];
        let (out, _) = normalize(
            "SF:/tmp/t/dx_coverlet/runfiles/_main/pkg/A.fs\nend_of_record\n",
            &prefixes,
        );
        assert_eq!(out, "SF:pkg/A.fs\nend_of_record\n");
    }

    #[test]
    fn handles_windows_paths() {
        let prefixes = vec!["C:\\t\\runfiles\\_main\\".to_string()];
        let (out, kept) = normalize(
            "SF:C:\\t\\runfiles\\_main\\pkg\\A.cs\r\nDA:1,1\r\nend_of_record\r\nSF:.\\pkg\\B.cs\nend_of_record\n",
            &prefixes,
        );
        assert_eq!(
            out,
            "SF:pkg/A.cs\nDA:1,1\nend_of_record\nSF:pkg/B.cs\nend_of_record\n"
        );
        assert_eq!(kept, 2);
    }

    #[test]
    fn drops_external_and_absolute_records() {
        let report = "SF:./external/nuget/X.cs\nDA:1,1\nend_of_record\nSF:/usr/src/Y.cs\nDA:2,0\nend_of_record\nSF:D:/z/Z.cs\nend_of_record\nSF:../up/W.cs\nend_of_record\nSF:./pkg/A.cs\nDA:3,1\nend_of_record\n";
        let (out, kept) = normalize(report, &[]);
        assert_eq!(out, "SF:pkg/A.cs\nDA:3,1\nend_of_record\n");
        assert_eq!(kept, 1);
    }

    #[test]
    fn counts_no_records_in_empty_report() {
        assert_eq!(normalize("", &[]), (String::new(), 0));
    }
}
