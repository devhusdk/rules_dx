use std::collections::BTreeMap;

use super::LcovError;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FileHits {
    pub lines: BTreeMap<u32, u64>,
    pub branches: BTreeMap<(u32, u32, u32), Option<u64>>,
}

fn merge_taken(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (None, None) => None,
        (Some(hits), None) => Some(hits),
        (None, Some(hits)) => Some(hits),
        (Some(a), Some(b)) => Some(a.max(b)),
    }
}

pub fn parse_lcov(report: &str) -> Result<BTreeMap<String, FileHits>, LcovError> {
    let mut files: BTreeMap<String, FileHits> = BTreeMap::new();
    let mut current: Option<String> = None;
    for raw in report.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line == "end_of_record" {
            current = None;
            continue;
        }
        if !line.contains(':') {
            continue;
        }
        match line.parse::<lcov::Record>() {
            Ok(lcov::Record::SourceFile { path }) => {
                let path_str = path.to_string_lossy().into_owned();
                if path_str.is_empty() {
                    return Err(LcovError::EmptySfPath);
                }
                current = Some(path_str.clone());
                files.entry(path_str).or_default();
            }
            Ok(lcov::Record::LineData {
                line: lineno,
                count: hits,
                ..
            }) => {
                let path = current.clone().ok_or_else(|| LcovError::DaOutsideSf {
                    line: line.to_string(),
                })?;
                if lineno == 0 {
                    return Err(LcovError::MalformedLineNumber {
                        path: path.clone(),
                        line: line.to_string(),
                    });
                }
                let slot = files
                    .entry(path)
                    .or_default()
                    .lines
                    .entry(lineno)
                    .or_insert(0);
                if hits > *slot {
                    *slot = hits;
                }
            }
            Ok(lcov::Record::BranchData {
                line: lineno,
                block,
                branch,
                taken,
            }) => {
                let path = current.clone().ok_or_else(|| LcovError::BranchOutsideSf {
                    line: line.to_string(),
                })?;
                if lineno == 0 {
                    return Err(LcovError::MalformedBranch {
                        path: path.clone(),
                        line: line.to_string(),
                    });
                }
                let key = (lineno, block, branch);
                let slot = files
                    .entry(path)
                    .or_default()
                    .branches
                    .entry(key)
                    .or_insert(None);
                *slot = merge_taken(*slot, taken);
            }
            Ok(lcov::Record::EndOfRecord) => {
                continue;
            }
            Ok(_) => {}
            Err(err) => {
                let kind = line.split_once(':').map(|(k, _)| k).unwrap_or("");
                if kind == "BRDA" {
                    if current.is_none() {
                        return Err(LcovError::BranchOutsideSf {
                            line: line.to_string(),
                        });
                    }
                    return Err(LcovError::MalformedBranch {
                        path: current.clone().unwrap_or_default(),
                        line: line.to_string(),
                    });
                }
                if kind != "DA" {
                    continue;
                }
                if current.is_none() {
                    return Err(LcovError::DaOutsideSf {
                        line: line.to_string(),
                    });
                }
                let path = current.clone().unwrap_or_default();
                match err {
                    lcov::record::ParseRecordError::ParseIntError("line", _)
                    | lcov::record::ParseRecordError::FieldNotFound("line") => {
                        return Err(LcovError::MalformedLineNumber {
                            path,
                            line: line.to_string(),
                        });
                    }
                    _ => {
                        return Err(LcovError::MalformedHitCount {
                            path,
                            line: line.to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(files)
}

pub fn merge_lcov_reports(documents: &[String]) -> Result<BTreeMap<String, FileHits>, LcovError> {
    let mut merged: BTreeMap<String, FileHits> = BTreeMap::new();
    for document in documents {
        let parsed = parse_lcov(document)?;
        for (path, hits) in parsed {
            let slot = merged.entry(path).or_default();
            for (line, count) in hits.lines {
                let cell = slot.lines.entry(line).or_insert(0);
                if count > *cell {
                    *cell = count;
                }
            }
            for (key, taken) in hits.branches {
                let cell = slot.branches.entry(key).or_insert(None);
                *cell = merge_taken(*cell, taken);
            }
        }
    }
    Ok(merged)
}

pub fn validate_lcov_report(report: &str) -> Result<(), LcovError> {
    let mut sections = 0u64;
    let mut open = false;
    let mut current: Option<String> = None;
    for raw in report.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line == "end_of_record" {
            if !open {
                return Err(LcovError::EndOfRecordOutsideSf);
            }
            open = false;
            current = None;
            continue;
        }
        if !line.contains(':') {
            continue;
        }
        match line.parse::<lcov::Record>() {
            Ok(lcov::Record::SourceFile { path }) => {
                let path_str = path.to_string_lossy().into_owned();
                if path_str.is_empty() {
                    return Err(LcovError::EmptySfPath);
                }
                if open {
                    return Err(LcovError::SfBeforeEndOfRecord);
                }
                open = true;
                sections += 1;
                current = Some(path_str);
            }
            Ok(lcov::Record::LineData { line: lineno, .. }) => {
                if !open {
                    return Err(LcovError::DaOutsideSf {
                        line: line.to_string(),
                    });
                }
                let path = current.clone().unwrap_or_default();
                if lineno == 0 {
                    return Err(LcovError::MalformedLineNumber {
                        path,
                        line: line.to_string(),
                    });
                }
            }
            Ok(lcov::Record::BranchData { line: lineno, .. }) => {
                if !open {
                    return Err(LcovError::BranchOutsideSf {
                        line: line.to_string(),
                    });
                }
                if lineno == 0 {
                    return Err(LcovError::MalformedBranch {
                        path: current.clone().unwrap_or_default(),
                        line: line.to_string(),
                    });
                }
            }
            Ok(lcov::Record::EndOfRecord) => {
                continue;
            }
            Ok(_) => {}
            Err(err) => {
                let kind = line.split_once(':').map(|(k, _)| k).unwrap_or("");
                if kind == "BRDA" {
                    if !open {
                        return Err(LcovError::BranchOutsideSf {
                            line: line.to_string(),
                        });
                    }
                    return Err(LcovError::MalformedBranch {
                        path: current.clone().unwrap_or_default(),
                        line: line.to_string(),
                    });
                }
                if kind != "DA" {
                    continue;
                }
                if !open {
                    return Err(LcovError::DaOutsideSf {
                        line: line.to_string(),
                    });
                }
                let path = current.clone().unwrap_or_default();
                match err {
                    lcov::record::ParseRecordError::ParseIntError("line", _)
                    | lcov::record::ParseRecordError::FieldNotFound("line") => {
                        return Err(LcovError::MalformedLineNumber {
                            path,
                            line: line.to_string(),
                        });
                    }
                    _ => {
                        return Err(LcovError::MalformedHitCount {
                            path,
                            line: line.to_string(),
                        });
                    }
                }
            }
        }
    }
    if sections == 0 {
        return Err(LcovError::NoSfRecords);
    }
    if open {
        return Err(LcovError::SfWithoutEndOfRecord);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_unions_duplicate_records() {
        let report = parse_lcov(
            "TN:\nSF:a.rs\nDA:1,0\nDA:2,3\nend_of_record\nSF:a.rs\nDA:1,2\nDA:3,1\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].lines[&1], 2);
        assert_eq!(report["a.rs"].lines[&2], 3);
        assert_eq!(report["a.rs"].lines[&3], 1);
    }

    #[test]
    fn parses_checksum_suffix_and_ignores_summaries() {
        let report = parse_lcov(
            "TN:\nSF:a.rs\nFN:1,main\nFNDA:1,main\nFNF:1\nFNH:1\nBRDA:2,0,0,0\nBRF:1\nBRH:0\nDA:1,1,abcd1234\nLH:1\nLF:1\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].lines.len(), 1);
        assert_eq!(report["a.rs"].lines[&1], 1);
        assert_eq!(report["a.rs"].branches.len(), 1);
        assert_eq!(report["a.rs"].branches[&(2, 0, 0)], Some(0));
    }

    #[test]
    fn retains_branch_taken_and_untaken_outcomes() {
        let report = parse_lcov(
            "SF:a.rs\nDA:1,1\nDA:2,1\nBRDA:1,0,0,3\nBRDA:1,0,1,-\nBRDA:2,1,0,0\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].branches[&(1, 0, 0)], Some(3));
        assert_eq!(report["a.rs"].branches[&(1, 0, 1)], None);
        assert_eq!(report["a.rs"].branches[&(2, 1, 0)], Some(0));
    }

    #[test]
    fn unions_duplicate_branch_records_with_max_taken() {
        let report = parse_lcov(
            "SF:a.rs\nDA:1,1\nBRDA:1,0,0,-\nend_of_record\nSF:a.rs\nDA:1,1\nBRDA:1,0,0,2\nBRDA:1,0,1,1\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].branches[&(1, 0, 0)], Some(2));
        assert_eq!(report["a.rs"].branches[&(1, 0, 1)], Some(1));
        let report = parse_lcov(
            "SF:a.rs\nDA:1,1\nBRDA:2,0,0,5\nBRDA:2,0,0,3\nBRDA:3,0,0,1\nBRDA:3,0,0,-\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].branches[&(2, 0, 0)], Some(5));
        assert_eq!(report["a.rs"].branches[&(3, 0, 0)], Some(1));
    }

    #[test]
    fn rejects_branch_outside_sf() {
        let err = parse_lcov("BRDA:1,0,0,1\n").unwrap_err();
        assert!(err.to_string().contains("BRDA"), "{err}");
        assert!(parse_lcov("BRDA:x,0,0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("BRDA:1,0,0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("BRDA:x,0,0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nBRDA:1,0,0,1\nend_of_record\n").is_ok());
    }

    #[test]
    fn rejects_malformed_branch_records() {
        assert!(parse_lcov("SF:a.rs\nBRDA:x,0,0,1\nend_of_record\n").is_err());
        assert!(parse_lcov("SF:a.rs\nBRDA:1,x,0,1\nend_of_record\n").is_err());
        assert!(parse_lcov("SF:a.rs\nBRDA:1,0\nend_of_record\n").is_err());
        assert!(parse_lcov("SF:a.rs\nBRDA:0,0,0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nBRDA:x,0,0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nBRDA:0,0,0,1\nend_of_record\n").is_err());
    }

    #[test]
    fn empty_report_parses_to_empty_map() {
        assert!(parse_lcov("").unwrap().is_empty());
        assert!(parse_lcov("\n  \n").unwrap().is_empty());
    }

    #[test]
    fn rejects_da_outside_sf() {
        let err = parse_lcov("DA:1,1\n").unwrap_err();
        assert!(err.to_string().contains("outside any SF"), "{err}");
    }

    #[test]
    fn rejects_malformed_da_numbers() {
        assert!(parse_lcov("SF:a.rs\nDA:x,1\nend_of_record\n").is_err());
        assert!(parse_lcov("SF:a.rs\nDA:1,x\nend_of_record\n").is_err());
        assert!(parse_lcov("SF:a.rs\nDA:1\nend_of_record\n").is_err());
    }

    #[test]
    fn rejects_zero_line_number() {
        assert!(parse_lcov("SF:a.rs\nDA:0,1\nend_of_record\n").is_err());
    }

    #[test]
    fn rejects_empty_sf_path() {
        assert!(parse_lcov("SF:\nDA:1,1\nend_of_record\n").is_err());
    }

    #[test]
    fn ignores_unknown_records_like_hand_parser() {
        let report = parse_lcov("SF:a.rs\nFOO:1,2\nDA:1,1\nend_of_record\n").unwrap();
        assert_eq!(report["a.rs"].lines[&1], 1);
    }

    #[test]
    fn strict_validator_requires_closed_sections() {
        assert!(validate_lcov_report("SF:/a.rs\nDA:1,1\nend_of_record\n").is_ok());
        assert!(validate_lcov_report("").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nDA:1,1\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nSF:/b.rs\n").is_err());
        assert!(validate_lcov_report("end_of_record\n").is_err());
        assert!(validate_lcov_report("DA:1,1\nend_of_record\n").is_err());
    }

    #[test]
    fn merges_documents_with_max_hits_winning() {
        let docs = [
            "SF:a.rs\nDA:1,0\nDA:2,1\nend_of_record\n".to_owned(),
            "SF:a.rs\nDA:1,3\nDA:3,0\nend_of_record\n".to_owned(),
        ];
        let merged = merge_lcov_reports(&docs).unwrap();
        assert_eq!(merged["a.rs"].lines[&1], 3);
        assert_eq!(merged["a.rs"].lines[&2], 1);
        assert_eq!(merged["a.rs"].lines[&3], 0);
        assert!(merge_lcov_reports(&[]).unwrap().is_empty());
        assert!(merge_lcov_reports(&["DA:1,1\n".to_owned()]).is_err());
    }

    #[test]
    fn merges_branch_documents_with_max_taken_winning() {
        let docs = [
            "SF:a.rs\nDA:1,1\nBRDA:1,0,0,-\nBRDA:1,0,1,1\nBRDA:3,0,0,4\nBRDA:4,0,0,2\nend_of_record\n"
                .to_owned(),
            "SF:a.rs\nDA:1,1\nBRDA:1,0,0,2\nBRDA:2,0,0,0\nBRDA:3,0,0,6\nBRDA:4,0,0,-\nend_of_record\n"
                .to_owned(),
        ];
        let merged = merge_lcov_reports(&docs).unwrap();
        assert_eq!(merged["a.rs"].branches[&(1, 0, 0)], Some(2));
        assert_eq!(merged["a.rs"].branches[&(1, 0, 1)], Some(1));
        assert_eq!(merged["a.rs"].branches[&(2, 0, 0)], Some(0));
        assert_eq!(merged["a.rs"].branches[&(3, 0, 0)], Some(6));
        assert_eq!(merged["a.rs"].branches[&(4, 0, 0)], Some(2));
        assert!(merge_lcov_reports(&["BRDA:1,0,0,1\n".to_owned()]).is_err());
    }

    #[test]
    fn skips_colonless_lines_and_suffix_end_markers() {
        let report = parse_lcov(
            "SF:a.rs\ngarbage line without colon\nDA:1,1\nend_of_record:trailing\nend_of_record\n",
        )
        .unwrap();
        assert_eq!(report["a.rs"].lines[&1], 1);
        assert!(validate_lcov_report(
            "SF:/a.rs\ngarbage line without colon\nDA:1,1\nend_of_record\n"
        )
        .is_ok());
    }

    #[test]
    fn rejects_malformed_da_outside_sf() {
        assert!(parse_lcov("DA:x,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("DA:x,1\nend_of_record\n").is_err());
    }

    #[test]
    fn validator_covers_empty_and_malformed_branches() {
        assert!(validate_lcov_report("SF:/a.rs\n\nDA:1,1\nend_of_record\n").is_ok());
        assert!(validate_lcov_report("SF:\nDA:1,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nDA:0,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nDA:x,1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nDA:1,x\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nDA:1\nend_of_record\n").is_err());
        assert!(validate_lcov_report("SF:/a.rs\nFOO:bad:bad\nDA:1,1\nend_of_record\n").is_ok());
        assert!(validate_lcov_report("SF:/a.rs\nFN:1,main\nDA:1,1\nend_of_record\n").is_ok());
        assert!(
            validate_lcov_report("SF:/a.rs\nDA:1,1\nend_of_record:trailing\nend_of_record\n")
                .is_ok()
        );
    }
}
