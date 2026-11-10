use crate::vuln::LockedPackage;
use dx_error::DxError;

pub const FPM_SET: &str = "fpm";

/// Reads the `[dependencies.<name>]` tables of an `fpm.lock`.
pub fn parse_fpm_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let mut out: Vec<LockedPackage> = Vec::new();
    let mut current: Option<(String, Option<String>)> = None;
    let mut finish = |entry: Option<(String, Option<String>)>| -> Result<(), DxError> {
        if let Some((name, version)) = entry {
            let version = version.ok_or_else(|| {
                DxError::failure(format!("invalid fpm.lock: no version for {name}"))
            })?;
            out.push(LockedPackage::new(FPM_SET, &name, &version));
        }
        Ok(())
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(table) = line.strip_prefix('[') {
            finish(current.take())?;
            let name = table
                .strip_suffix(']')
                .and_then(|inner| inner.strip_prefix("dependencies."))
                .ok_or_else(|| {
                    DxError::failure(format!("invalid fpm.lock: unsupported table {line}"))
                })?;
            current = Some((name.trim_matches('"').to_owned(), None));
        } else if let Some((key, value)) = line.split_once('=')
            && key.trim() == "version"
            && let Some((_, version)) = current.as_mut()
        {
            *version = Some(value.trim().trim_matches('"').to_owned());
        }
    }
    finish(current.take())?;
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
