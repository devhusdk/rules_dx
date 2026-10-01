use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

pub fn normalize_go(name: &str) -> String {
    name.to_lowercase()
}

pub fn parse_go_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let file = dx_gomod::parse_mod(&text).map_err(DepcheckError::ManifestParse)?;
    let mut deps = BTreeMap::new();
    for requirement in file.require {
        let marker = requirement.marker.unwrap_or_default().to_lowercase();
        deps.insert(
            normalize_go(&requirement.module),
            DepInfo {
                spec: requirement.version,
                category: if is_dev(&marker) {
                    "dev".to_owned()
                } else {
                    "prod".to_owned()
                },
                optional: flagged(&marker, "depcheck:optional", "optional"),
                platform: flagged(&marker, "depcheck:platform", "platform"),
                raw: requirement.module,
                peer: false,
                sha256: String::new(),
            },
        );
    }
    Ok(deps)
}

pub fn parse_go_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let mut pkgs = BTreeMap::new();
    for (module, version) in dx_gomod::parse_sum(&text) {
        pkgs.entry(normalize_go(&module)).or_insert(version);
    }
    Ok(pkgs)
}

fn is_dev(marker: &str) -> bool {
    flagged(marker, "depcheck:test", "test")
}

fn flagged(marker: &str, prefixed: &str, word: &str) -> bool {
    marker.contains(prefixed) || marker.split_whitespace().any(|token| token == word)
}
