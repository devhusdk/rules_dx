//! Release trust inputs: cells, asset names, the checksum manifest and the gh verify call.

use std::collections::BTreeMap;
use std::path::Path;

pub use dx_release_tools::{
    PROVENANCE_BUILDER_RELEASE, RELEASE_REPOSITORY, SIGNING_ISSUER_DEFAULT,
};

/// Checksum manifest every release publishes: the raw `dx-<cell>` binaries and the archives.
pub const MANIFEST_NAME: &str = "dx-sha256sums.txt";
/// Provenance bundle whose subjects are every line of the manifest (raw binaries and archives).
pub const BUNDLE_NAME: &str = "dx-sha256sums.txt.sigstore.json";

/// One release matrix cell; mirrors deploy/release/matrix.bzl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub name: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub suffix: &'static str,
    pub exe: &'static str,
}

impl Cell {
    /// The release archive asset name for this cell.
    pub fn archive_name(&self) -> String {
        format!("{}{}", self.name, self.suffix)
    }
}

pub const CELLS: [Cell; 5] = [
    Cell {
        name: "dx-linux-x86_64",
        os: "linux",
        arch: "x86_64",
        suffix: ".tar.gz",
        exe: "dx",
    },
    Cell {
        name: "dx-linux-arm64",
        os: "linux",
        arch: "arm64",
        suffix: ".tar.gz",
        exe: "dx",
    },
    Cell {
        name: "dx-macos-arm64",
        os: "macos",
        arch: "arm64",
        suffix: ".tar.gz",
        exe: "dx",
    },
    Cell {
        name: "dx-windows-x86_64",
        os: "windows",
        arch: "x86_64",
        suffix: ".zip",
        exe: "dx.exe",
    },
    Cell {
        name: "dx-windows-arm64",
        os: "windows",
        arch: "arm64",
        suffix: ".zip",
        exe: "dx.exe",
    },
];

/// The cell for an `std::env::consts` OS and ARCH pair.
pub fn cell_for(os: &str, arch: &str) -> Result<&'static Cell, String> {
    let arch = if arch == "aarch64" { "arm64" } else { arch };
    if let Some(cell) = CELLS.iter().find(|cell| cell.os == os && cell.arch == arch) {
        return Ok(cell);
    }
    if os == "macos" && arch == "x86_64" {
        return Err(
            "dx releases have no macOS x86_64 build; use an arm64 Mac or build dx from source"
                .to_owned(),
        );
    }
    Err(format!("dx releases have no build for {os} {arch}"))
}

/// The cell this process runs on.
pub fn host_cell() -> Result<&'static Cell, String> {
    cell_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn numeric_ok(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

/// Accepts `v` plus a strict semver core with an optional pre-release; returns the version.
pub fn validate_tag(tag: &str) -> Result<&str, String> {
    let refuse = || format!("tag {tag:?} must be 'v' followed by a semver version, like v1.2.3");
    let version = tag.strip_prefix('v').ok_or_else(refuse)?;
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| numeric_ok(part)) {
        return Err(refuse());
    }
    if let Some(pre) = pre {
        for ident in pre.split('.') {
            let shape = !ident.is_empty()
                && ident
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-');
            let numeric = ident.bytes().all(|b| b.is_ascii_digit());
            if !shape || (numeric && !numeric_ok(ident)) {
                return Err(refuse());
            }
        }
    }
    Ok(version)
}

/// Parses `<64 lowercase hex>  <asset>` lines; only known archive or raw binary names, no
/// duplicates. Raw binary lines are accepted but not returned: install uses archives only.
pub fn parse_sha256sums(text: &str) -> Result<BTreeMap<String, String>, String> {
    let known: Vec<String> = CELLS.iter().map(Cell::archive_name).collect();
    let mut sums = BTreeMap::new();
    let mut seen_raw = std::collections::BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let at = index + 1;
        let Some((digest, name)) = line.split_once("  ") else {
            return Err(format!(
                "{MANIFEST_NAME} line {at}: want '<sha256>  <asset>'"
            ));
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!(
                "{MANIFEST_NAME} line {at}: digest must be 64 lowercase hex characters"
            ));
        }
        let raw = CELLS.iter().any(|cell| cell.name == name);
        if !raw && !known.iter().any(|known| known == name) {
            return Err(format!("{MANIFEST_NAME} line {at}: unknown asset {name:?}"));
        }
        if raw {
            if !seen_raw.insert(name.to_owned()) {
                return Err(format!(
                    "{MANIFEST_NAME} line {at}: duplicate asset {name:?}"
                ));
            }
            continue;
        }
        if sums.insert(name.to_owned(), digest.to_owned()).is_some() {
            return Err(format!(
                "{MANIFEST_NAME} line {at}: duplicate asset {name:?}"
            ));
        }
    }
    if sums.is_empty() {
        return Err(format!("{MANIFEST_NAME} lists no assets"));
    }
    Ok(sums)
}

/// `owner/repo` for the release repository.
pub fn release_repo_slug() -> &'static str {
    RELEASE_REPOSITORY
        .strip_prefix("https://github.com/")
        .unwrap_or(RELEASE_REPOSITORY)
}

/// The exact `gh attestation verify` argv for one archive; `tag` must be validated.
pub fn gh_verify_argv(gh: &str, subject: &Path, bundle: &Path, tag: &str) -> Vec<String> {
    vec![
        gh.to_owned(),
        "attestation".to_owned(),
        "verify".to_owned(),
        subject.to_string_lossy().into_owned(),
        "--bundle".to_owned(),
        bundle.to_string_lossy().into_owned(),
        "--repo".to_owned(),
        release_repo_slug().to_owned(),
        "--cert-identity".to_owned(),
        format!("{PROVENANCE_BUILDER_RELEASE}@refs/tags/{tag}"),
        "--cert-oidc-issuer".to_owned(),
        SIGNING_ISSUER_DEFAULT.to_owned(),
        "--source-ref".to_owned(),
        format!("refs/tags/{tag}"),
        "--deny-self-hosted-runners".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gh_verify_argv_is_exact() {
        let argv = gh_verify_argv(
            "gh",
            Path::new("/d/dx-linux-x86_64.tar.gz"),
            Path::new("/d/dx-sha256sums.txt.sigstore.json"),
            "v1.2.3",
        );
        assert_eq!(
            argv,
            [
                "gh",
                "attestation",
                "verify",
                "/d/dx-linux-x86_64.tar.gz",
                "--bundle",
                "/d/dx-sha256sums.txt.sigstore.json",
                "--repo",
                "devhusdk/rules_dx",
                "--cert-identity",
                "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/v1.2.3",
                "--cert-oidc-issuer",
                "https://token.actions.githubusercontent.com",
                "--source-ref",
                "refs/tags/v1.2.3",
                "--deny-self-hosted-runners",
            ]
        );
    }

    #[test]
    fn tags_are_strict() {
        assert_eq!(validate_tag("v1.2.3"), Ok("1.2.3"));
        assert_eq!(validate_tag("v0.10.0-rc.1"), Ok("0.10.0-rc.1"));
        for bad in [
            "1.2.3",
            "v1.2",
            "v1.2.3.4",
            "v01.2.3",
            "v1/../x",
            "v1.2.3 ",
            "v1.2.3 --x",
            "v1.2.3+build",
            "v1.2.3-",
            "v1.2.3-rc..1",
            "v1.2.3-01",
            "V1.2.3",
            "",
        ] {
            assert!(validate_tag(bad).is_err(), "{bad:?} must be refused");
        }
    }

    fn line(name: &str) -> String {
        format!("{}  {name}", "a".repeat(64))
    }

    #[test]
    fn manifest_parses_known_assets() {
        let text = format!(
            "{}\n{}\n",
            line("dx-linux-x86_64.tar.gz"),
            line("dx-windows-arm64.zip")
        );
        let sums = parse_sha256sums(&text).expect("parse");
        assert_eq!(sums.len(), 2);
        assert_eq!(sums["dx-windows-arm64.zip"], "a".repeat(64));
    }

    #[test]
    fn manifest_accepts_raw_binaries_and_archives() {
        let mut lines = Vec::new();
        for cell in CELLS {
            lines.push(line(cell.name));
            lines.push(line(&cell.archive_name()));
        }
        let sums = parse_sha256sums(&format!("{}\n", lines.join("\n"))).expect("parse");
        assert_eq!(sums.len(), 5);
        assert!(!sums.contains_key("dx-linux-x86_64"));
        for text in [
            format!("{}\n{}", line("dx-linux-x86_64"), line("dx-linux-x86_64")),
            line("dx-linux-x86_64.zip"),
            line("dx-windows-x86_64.tar.gz"),
            line("dx-macos-x86_64"),
        ] {
            assert!(parse_sha256sums(&text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn manifest_rejects_bad_lines() {
        let upper = format!("{}  dx-linux-x86_64.tar.gz", "A".repeat(64));
        let short = format!("{}  dx-linux-x86_64.tar.gz", "a".repeat(63));
        let star = format!("{} *dx-linux-x86_64.tar.gz", "a".repeat(64));
        let dup = format!(
            "{}\n{}",
            line("dx-linux-x86_64.tar.gz"),
            line("dx-linux-x86_64.tar.gz")
        );
        let unknown = line("evil.tar.gz");
        let path = line("../dx-linux-x86_64.tar.gz");
        for (text, want) in [
            (upper.as_str(), "lowercase hex"),
            (short.as_str(), "lowercase hex"),
            (star.as_str(), "want '<sha256>  <asset>'"),
            (dup.as_str(), "duplicate"),
            (unknown.as_str(), "unknown asset"),
            (path.as_str(), "unknown asset"),
            ("", "lists no assets"),
        ] {
            let error = parse_sha256sums(text).expect_err(text);
            assert!(error.contains(want), "{text:?}: {error}");
        }
    }

    #[test]
    fn host_cells_map_consts() {
        assert_eq!(
            cell_for("linux", "aarch64").expect("cell").name,
            "dx-linux-arm64"
        );
        assert_eq!(
            cell_for("windows", "x86_64").expect("cell").archive_name(),
            "dx-windows-x86_64.zip"
        );
        assert_eq!(cell_for("macos", "aarch64").expect("cell").exe, "dx");
        assert!(cell_for("macos", "x86_64")
            .expect_err("no intel mac")
            .contains("no macOS x86_64 build"));
        assert!(cell_for("freebsd", "x86_64").is_err());
        assert!(host_cell().is_ok() || std::env::consts::OS == "macos");
    }

    #[test]
    fn cells_follow_the_release_matrix() {
        let matrix = include_str!("../../release/matrix.bzl");
        for cell in CELLS {
            assert_eq!(cell.name, format!("dx-{}-{}", cell.os, cell.arch));
            assert!(
                matrix.contains(&format!("_cell(\"{}\", \"{}\",", cell.os, cell.arch)),
                "{} missing from matrix.bzl",
                cell.name
            );
        }
    }
}
