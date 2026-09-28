use super::{ExceptionProblem, FindingRef, RiskException};

pub fn is_obsolete(exception: &RiskException, findings: &[FindingRef]) -> bool {
    !findings.iter().any(|finding| {
        finding.advisory == exception.advisory && finding.package == exception.package
    })
}

pub fn check_applies(
    exception: &RiskException,
    findings: &[FindingRef],
) -> Result<(), ExceptionProblem> {
    if is_obsolete(exception, findings) {
        return Err(ExceptionProblem::Obsolete {
            advisory: exception.advisory.clone(),
            package: exception.package.clone(),
        });
    }
    Ok(())
}
