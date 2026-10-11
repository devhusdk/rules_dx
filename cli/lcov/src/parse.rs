use std::collections::BTreeMap;

use super::LcovError;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FileHits {
    pub lines: BTreeMap<u32, u64>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FileHits {
    pub lines: BTreeMap<u32, u64>,
    pub branches: BTreeMap<(u32, u32, u32), u64>,
    pub branch_observed: bool,
}

#[derive(Debug, Default)]
struct SectionBranches {
    records: u64,
    taken: u64,
    found: Option<u32>,
    hit: Option<u32>,
    observed: bool,
}

fn check_section_branches(path: &str, section: &SectionBranches) -> Result<(), LcovError> {
    if !section.observed {
        return Ok(());
    }
    match (section.found, section.hit) {
        (None, None) => Ok(()),
        (Some(found), Some(hit)) => {
            if u64::from(found) != section.records {
                return Err(LcovError::BranchSummaryMismatch {
                    path: path.to_string(),
                    detail: format!(
                        "BRF:{found} names {found} branches but {} BRDA records present",
                        section.records
                    ),
                });
            }
            if u64::from(hit) != section.taken {
                return Err(LcovError::BranchSummaryMismatch {
                    path: path.to_string(),
                    detail: format!(
                        "BRH:{hit} names {hit} hits but {} BRDA branches taken",
                        section.taken
                    ),
                });
            }
            Ok(())
        }
        _ => Err(LcovError::BranchSummaryMismatch {
            path: path.to_string(),
            detail: "branch summary without a complete BRF/BRH pair".to_string(),
        }),
    }
}

fn open_path(open: &Option<(String, SectionBranches)>) -> Option<String> {
    open.as_ref().map(|(path, _)| path.clone())
}

pub fn parse_lcov(report: &str) -> Result<BTreeMap<String, FileHits>, LcovError> {
    let mut files: BTreeMap<String, FileHits> = BTreeMap::new();
    let mut open: Option<(String, SectionBranches)> = None;
    for raw in report.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line == "end_of_record" {
            if let Some((path, section)) = open.take() {
                check_section_branches(&path, &section)?;
            }
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
                if let Some((previous, section)) = open.take() {
                    check_section_branches(&previous, &section)?;
                }
                files.entry(path_str.clone()).or_default();
                open = Some((path_str, SectionBranches::default()));
            }
            Ok(lcov::Record::LineData {
                line: lineno,
                count: hits,
                ..
            }) => {
                let path = open_path(&open).ok_or_else(|| LcovError::DaOutsideSf {
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
                let path = open_path(&open).ok_or_else(|| LcovError::BrdaOutsideSf {
                    line: line.to_string(),
                })?;
                if lineno == 0 {
                    return Err(LcovError::MalformedBranch {
                        path: path.clone(),
                        line: line.to_string(),
                    });
                }
                let taken_count = taken.unwrap_or(0);
                if let Some((_, section)) = open.as_mut() {
                    section.observed = true;
                    section.records += 1;
                    if taken_count > 0 {
                        section.taken += 1;
                    }
                }
                let slot = files.entry(path).or_default();
                slot.branch_observed = true;
                let cell = slot
                    .branches
                    .entry((lineno, block, branch))
                    .or_insert(0);
                if taken_count > *cell {
                    *cell = taken_count;
                }
            }
            Ok(lcov::Record::BranchesFound { found }) => {
                let path = open_path(&open).ok_or_else(|| LcovError::BrdaOutsideSf {
                    line: line.to_string(),
                })?;
                files.entry(path).or_default().branch_observed = true;
                if let Some((_, section)) = open.as_mut() {
                    section.observed = true;
                    section.found = Some(found);
                }
            }
            Ok(lcov::Record::BranchesHit { hit }) => {
                let path = open_path(&open).ok_or_else(|| LcovError::BrdaOutsideSf {
                    line: line.to_string(),
                })?;
                files.entry(path).or_default().branch_observed = true;
                if let Some((_, section)) = open.as_mut() {
                    section.observed = true;
                    section.hit = Some(hit);
                }
            }
            Ok(lcov::Record::EndOfRecord) => {
                continue;
            }
            Ok(_) => {}
            Err(err) => {
                let kind = line.split_once(':').map(|(k, _)| k).unwrap_or("");
                if kind == "DA" {
                    if open_path(&open).is_none() {
                        return Err(LcovError::DaOutsideSf {
                            line: line.to_string(),
                        });
                    }
                    let path = open_path(&open).unwrap_or_default();
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
                if kind == "BRDA" || kind == "BRF" || kind == "BRH" {
                    match open_path(&open) {
                        None => {
                            return Err(LcovError::BrdaOutsideSf {
                                line: line.to_string(),
                            });
                        }
                        Some(path) => {
                            return Err(LcovError::MalformedBranch {
                                path,
                                line: line.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    if let Some((path, section)) = open.take() {
        check_section_branches(&path, &section)?;
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
            if hits.branch_observed {
                slot.branch_observed = true;
            }
            for (key, taken) in hits.branches {
                let cell = slot.branches.entry(key).or_insert(0);
                if taken > *cell {
                    *cell = taken;
                }
            }
        }
    }
    Ok(merged)
}

pub fn validate_lcov_report(report: &str) -> Result<(), LcovError> {
    let mut sections = 0u64;
    let mut open = false;
    let mut current: Option<String> = None;
    let mut section = SectionBranches::default();
    for raw in report.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line == "end_of_record" {
            if !open {
                return Err(LcovError::EndOfRecordOutsideSf);
            }
            check_section_branches(&current.clone().unwrap_or_default(), &section)?;
            open = false;
            current = None;
            section = SectionBranches::default();
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
            Ok(lcov::Record::BranchData {
                line: lineno,
                taken,
                ..
            }) => {
                if !open {
                    return Err(LcovError::BrdaOutsideSf {
                        line: line.to_string(),
                    });
                }
                let path = current.clone().unwrap_or_default();
                if lineno == 0 {
                    return Err(LcovError::MalformedBranch {
                        path,
                        line: line.to_string(),
                    });
                }
                section.observed = true;
                section.records += 1;
                if taken.unwrap_or(0) > 0 {
                    section.taken += 1;
                }
            }
            Ok(lcov::Record::BranchesFound { found }) => {
                if !open {
                    return Err(LcovError::BrdaOutsideSf {
                        line: line.to_string(),
                    });
                }
                section.observed = true;
                section.found = Some(found);
            }
            Ok(lcov::Record::BranchesHit { hit }) => {
                if !open {
                    return Err(LcovError::BrdaOutsideSf {
                        line: line.to_string(),
                    });
                }
                section.observed = true;
                section.hit = Some(hit);
            }
            Ok(lcov::Record::EndOfRecord) => {
                continue;
            }
            Ok(_) => {}
            Err(err) => {
                let kind = line.split_once(':').map(|(k, _)| k).unwrap_or("");
                if kind == "DA" {
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
                if kind == "BRDA" || kind == "BRF" || kind == "BRH" {
                    if !open {
                        return Err(LcovError::BrdaOutsideSf {
                            line: line.to_string(),
                        });
                    }
                    let path = current.clone().unwrap_or_default();
                    return Err(LcovError::MalformedBranch {
                        path,
                        line: line.to_string(),
                    });
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
