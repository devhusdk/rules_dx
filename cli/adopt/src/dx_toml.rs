//! Edits to the workspace `dx.toml` that keep its comments and layout.

use std::path::Path;

use crate::AdoptError;

const DX_TOML: &str = "dx.toml";

/// Sets the top-level `tier` key, creating `dx.toml` when it is missing.
/// Returns whether the file changed.
pub fn set_tier(root: &Path, tier: &str) -> Result<bool, AdoptError> {
    let path = root.join(DX_TOML);
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(AdoptError::ReadFile {
                path: DX_TOML.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    let mut doc: toml_edit::DocumentMut = old
        .as_deref()
        .unwrap_or("schema_version = 1\n")
        .parse()
        .map_err(|error: toml_edit::TomlError| AdoptError::ReadFile {
            path: DX_TOML.to_owned(),
            detail: error.to_string(),
        })?;
    doc["tier"] = toml_edit::value(tier);
    let new = doc.to_string();
    if old.as_deref() == Some(new.as_str()) {
        return Ok(false);
    }
    std::fs::create_dir_all(root).map_err(|error| AdoptError::CreateParent {
        parent: root.display().to_string(),
        detail: error.to_string(),
    })?;
    dx_atomic_fs::write_atomic(&path, new.as_bytes()).map_err(|error| AdoptError::WriteFile {
        path: DX_TOML.to_owned(),
        detail: error.to_string(),
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_the_file_then_edits_only_the_tier() {
        let scratch = dx_test_scratch::scratch("dx-toml-tier-");
        let root = scratch.path();
        assert!(set_tier(root, "strict").expect("create"));
        let read = || std::fs::read_to_string(root.join(DX_TOML)).expect("read");
        assert_eq!(read(), "schema_version = 1\ntier = \"strict\"\n");
        assert!(!set_tier(root, "strict").expect("same"));
        std::fs::write(
            root.join(DX_TOML),
            "# mine\nschema_version = 1\ntier = \"strict\"\n\n[families.python]\ndisabled = true\n",
        )
        .expect("write");
        assert!(set_tier(root, "standard").expect("edit"));
        assert_eq!(
            read(),
            "# mine\nschema_version = 1\ntier = \"standard\"\n\n[families.python]\ndisabled = true\n"
        );
    }
}
