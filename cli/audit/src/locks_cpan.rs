use crate::locks::indent_of;
use crate::vuln::LockedPackage;
use dx_error::DxError;

pub fn parse_cpanfile_snapshot(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let mut in_distributions = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        if raw.trim().is_empty() || raw.starts_with('#') {
            continue;
        }
        if indent_of(raw) == 0 {
            in_distributions = raw.trim() == "DISTRIBUTIONS";
            continue;
        }
        if in_distributions && indent_of(raw) == 2 {
            let (name, version) = split_distribution(raw.trim()).ok_or_else(|| {
                DxError::failure(format!(
                    "invalid cpanfile.snapshot distribution: {}",
                    raw.trim()
                ))
            })?;
            out.push(LockedPackage::new("cpan", name, version));
        }
    }
    if out.is_empty() {
        return Err(DxError::failure(
            "invalid cpanfile.snapshot: missing DISTRIBUTIONS entries",
        ));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn split_distribution(entry: &str) -> Option<(String, String)> {
    let at = entry.rmatch_indices('-').find_map(|(index, _)| {
        let numeric = entry[index + 1..]
            .trim_start_matches('v')
            .starts_with(|c: char| c.is_ascii_digit());
        (index > 0 && numeric).then_some(index)
    })?;
    Some((entry[..at].to_owned(), entry[at + 1..].to_owned()))
}
