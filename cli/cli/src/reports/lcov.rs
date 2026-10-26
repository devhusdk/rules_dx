use dx_lcov::{find_ignores, is_covered_language, is_ignored, merge_lcov_reports};

use super::ReportError;

pub fn validate_lcov(bytes: &[u8]) -> Result<(), ReportError> {
    let invalid = |detail: String| ReportError::InvalidLcov { detail };
    let text =
        std::str::from_utf8(bytes).map_err(|e| invalid(format!("LCOV is not UTF-8: {e}")))?;
    dx_lcov::validate_lcov_report(text).map_err(|e| invalid(e.to_string()))
}

pub fn coverage_line_rate(
    documents: &[String],
    load: &dyn Fn(&str) -> Option<String>,
) -> Result<(u64, u64), ReportError> {
    let invalid = |e: dx_lcov::LcovError| ReportError::InvalidLcov {
        detail: e.to_string(),
    };
    let merged = merge_lcov_reports(documents).map_err(&invalid)?;
    let mut covered = 0u64;
    let mut eligible = 0u64;
    for (path, hits) in &merged {
        let mut ignores = None;
        if is_covered_language(path) {
            if let Some(source) = load(path) {
                ignores = Some(find_ignores(path, &source).map_err(&invalid)?);
            }
        }
        for (line, count) in &hits.lines {
            let ignored = ignores
                .as_ref()
                .is_some_and(|valid| is_ignored(valid, *line));
            if ignored {
                continue;
            }
            eligible += 1;
            if *count > 0 {
                covered += 1;
            }
        }
    }
    Ok((covered, eligible))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcov_validation_covers_all_branches() {
        let good = "SF:/a.rs\nDA:1,1\nend_of_record\n";
        assert_eq!(validate_lcov(good.as_bytes()), Ok(()));
        assert!(validate_lcov(&[0xff]).is_err());
        assert!(validate_lcov(b"DA:1,1\nend_of_record\n").is_err());
        assert!(validate_lcov(b"SF:\nDA:1,1\nend_of_record\n").is_err());
        assert!(validate_lcov(b"SF:/a.rs\nSF:/b.rs\n").is_err());
        assert!(validate_lcov(b"SF:/a.rs\nDA:bad,1\nend_of_record\n").is_err());
        assert!(validate_lcov(b"SF:/a.rs\nDA:0,1\nend_of_record\n").is_err());
        assert!(validate_lcov(b"SF:/a.rs\nDA:1,bad\nend_of_record\n").is_err());
        assert!(validate_lcov(b"end_of_record\n").is_err());
        assert!(validate_lcov(b"SF:/a.rs\nDA:1,1\n").is_err());
        assert!(validate_lcov(b"").is_err());
        let with_noise = "\nSF:/a.rs\nFN:1,fn\nBRDA:1,0,0,1\nDA:1,1\nend_of_record\n";
        assert_eq!(validate_lcov(with_noise.as_bytes()), Ok(()));
    }

    #[test]
    fn lcov_validation_accepts_matching_branch_summary() {
        let matching =
            "SF:/a.rs\nDA:1,1\nBRDA:1,0,0,1\nBRDA:1,0,1,-\nBRF:2\nBRH:1\nend_of_record\n";
        assert_eq!(validate_lcov(matching.as_bytes()), Ok(()));
    }

    #[test]
    fn lcov_validation_rejects_incomplete_branch_summary() {
        let short = "SF:/a.rs\nDA:1,1\nBRDA:1,0,0,1\nBRF:2\nend_of_record\n";
        assert!(validate_lcov(short.as_bytes()).is_err());
        let malformed = "SF:/a.rs\nDA:1,1\nBRDA:1,0,0,x\nend_of_record\n";
        assert!(validate_lcov(malformed.as_bytes()).is_err());
        let outside = "BRDA:1,0,0,1\nend_of_record\n";
        assert!(validate_lcov(outside.as_bytes()).is_err());
    }

    fn rate(documents: &[&str], sources: &[(&str, &str)]) -> Result<(u64, u64), ReportError> {
        let owned: Vec<String> = documents.iter().map(ToString::to_string).collect();
        coverage_line_rate(&owned, &|path| {
            sources
                .iter()
                .find(|(name, _)| *name == path)
                .map(|(_, text)| (*text).to_owned())
        })
    }

    #[test]
    fn coverage_rate_counts_hits_and_uncovered() {
        let documents = ["SF:src/a.py\nDA:1,1\nDA:2,0\nDA:3,5\nend_of_record\n"];
        assert_eq!(rate(&documents, &[]), Ok((2, 3)));
    }

    #[test]
    fn coverage_rate_unions_duplicate_records_with_max_hits() {
        let documents = [
            "SF:src/a.rs\nDA:1,0\nDA:2,1\nend_of_record\n".to_owned(),
            "SF:src/a.rs\nDA:1,3\nDA:3,0\nend_of_record\n".to_owned(),
        ];
        let source = "fn a() {}\nfn b() {}\nfn c() {}\n";
        assert_eq!(
            coverage_line_rate(&documents, &|_| Some(source.to_owned())),
            Ok((2, 3))
        );
    }

    #[test]
    fn coverage_rate_honors_exclusion_markers() {
        let documents = ["SF:src/a.rs\nDA:1,1\nDA:2,0\nDA:3,0\nend_of_record\n"];
        let source = "fn a() {}\n// LCOV_EXCL_LINE - reason: generated, issue: 1055.\nfn c() {}\n";
        assert_eq!(rate(&documents, &[("src/a.rs", source)]), Ok((1, 2)));
    }

    #[test]
    fn coverage_rate_counts_unloadable_sources_raw() {
        let documents = ["SF:src/a.rs\nDA:1,1\nDA:2,0\nend_of_record\n"];
        assert_eq!(rate(&documents, &[]), Ok((1, 2)));
    }

    #[test]
    fn coverage_rate_rejects_invalid_markers() {
        let documents = ["SF:src/a.rs\nDA:1,1\nend_of_record\n"];
        let source = "// LCOV_EXCL_LINE\nfn a() {}\n";
        assert!(rate(&documents, &[("src/a.rs", source)]).is_err());
    }

    #[test]
    fn coverage_rate_rejects_malformed_lcov() {
        let documents = ["DA:1,1\nend_of_record\n"];
        assert!(rate(&documents, &[]).is_err());
    }

    #[test]
    fn coverage_rate_counts_wrapped_jvm_dotnet_sources() {
        for (path, source) in [
            ("src/A.java", "public class A {}\n"),
            ("src/A.kt", "fun f() = 1\n"),
            ("src/A.scala", "object A\n"),
            ("src/A.cs", "public static class A {}\n"),
            ("src/A.fs", "module A\n"),
            ("src/A.fsi", "module A\n"),
        ] {
            let document = format!("SF:{path}\nDA:1,1\nDA:2,0\nend_of_record\n");
            assert_eq!(
                rate(&[document.as_str()], &[(path, source)]),
                Ok((1, 2)),
                "{path}"
            );
        }
    }

    #[test]
    fn coverage_rate_honors_wrapped_lang_markers() {
        let documents = ["SF:src/A.java\nDA:1,1\nDA:2,0\nDA:3,0\nend_of_record\n"];
        let source = "public class A {\n// LCOV_EXCL_LINE - reason: generated, issue: 1055.\n}\n";
        assert_eq!(rate(&documents, &[("src/A.java", source)]), Ok((1, 2)));
    }

    #[test]
    fn coverage_rate_honors_markers_for_rb_kts_erl() {
        for (path, source) in [
            (
                "src/a.rb",
                "x = 1\ny = 2 # LCOV_EXCL_LINE - reason: fixture, issue: 1055.\n",
            ),
            (
                "src/a.kts",
                "val x = 1\nval y = 2 // LCOV_EXCL_LINE - reason: fixture, issue: 1055.\n",
            ),
            (
                "src/a.erl",
                "x.\ny. % LCOV_EXCL_LINE - reason: fixture, issue: 1055.\n",
            ),
        ] {
            let document = format!("SF:{path}\nDA:1,1\nDA:2,0\nend_of_record\n");
            assert_eq!(
                rate(&[document.as_str()], &[(path, source)]),
                Ok((1, 1)),
                "{path}"
            );
        }
    }

    #[test]
    fn coverage_rate_rejects_bare_markers_for_rb_kts_erl() {
        for (path, source) in [
            ("src/a.rb", "x = 1\ny = 2 # LCOV_EXCL_LINE\n"),
            ("src/a.kts", "val x = 1\nval y = 2 // LCOV_EXCL_LINE\n"),
            ("src/a.erl", "x.\ny. % LCOV_EXCL_LINE\n"),
        ] {
            let document = format!("SF:{path}\nDA:1,1\nDA:2,0\nend_of_record\n");
            assert!(
                rate(&[document.as_str()], &[(path, source)]).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn coverage_rate_counts_modern_js_ts_extensions() {
        for path in ["src/a.mjs", "src/a.cjs", "src/a.mts", "src/a.cts"] {
            let document = format!("SF:{path}\nDA:1,1\nDA:2,0\nend_of_record\n");
            assert_eq!(rate(&[document.as_str()], &[]), Ok((1, 2)), "{path}");
        }
    }
}
