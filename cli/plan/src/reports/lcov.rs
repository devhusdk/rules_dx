use super::ReportError;

pub fn validate_lcov(bytes: &[u8]) -> Result<(), ReportError> {
    let invalid = |detail: String| ReportError::InvalidLcov { detail };
    let text =
        std::str::from_utf8(bytes).map_err(|e| invalid(format!("LCOV is not UTF-8: {e}")))?;
    dx_lcov::validate_lcov_report(text).map_err(|e| invalid(e.to_string()))
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
}
