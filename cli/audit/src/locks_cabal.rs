use crate::vuln::LockedPackage;

/// Reads the `any.<name> ==<version>` entries of a cabal.project.freeze `constraints:` field.
/// Flag entries and `installed` boot packages carry no version and are skipped.
pub fn parse_cabal_freeze(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut constraints = String::new();
    let mut inside = false;
    for line in text.lines() {
        let indented = line.starts_with([' ', '\t']);
        if let Some(rest) = line.strip_prefix("constraints:") {
            inside = true;
            constraints.push_str(rest);
        } else if inside && indented {
            constraints.push_str(line);
        } else {
            inside = false;
        }
        constraints.push('\n');
    }
    if !text.lines().any(|line| line.starts_with("constraints:")) {
        return Err("invalid cabal.project.freeze: missing constraints".to_owned());
    }
    let mut out = Vec::new();
    for entry in constraints.split(',') {
        let entry = entry.trim();
        let entry = entry.strip_prefix("any.").unwrap_or(entry);
        let Some((name, version)) = entry.split_once("==") else {
            continue;
        };
        let (name, version) = (name.trim(), version.trim());
        if name.is_empty() || version.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        out.push(LockedPackage::new("hackage", name, version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(text: &str) -> Vec<(String, String)> {
        parse_cabal_freeze(text)
            .expect("parsed")
            .into_iter()
            .map(|package| (package.name, package.version))
            .collect()
    }

    #[test]
    fn multi_line_constraints_are_read() {
        let text = "active-repositories: hackage.haskell.org:merge\nconstraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21,\n             any.split ==0.2.5\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n";
        assert_eq!(
            pins(text),
            [
                ("base".to_owned(), "4.20.2.0".to_owned()),
                ("safe".to_owned(), "0.3.21".to_owned()),
                ("split".to_owned(), "0.2.5".to_owned())
            ]
        );
    }

    #[test]
    fn flags_and_installed_packages_are_skipped() {
        let text = "constraints: any.aeson ==2.2.3.0,\n             aeson +ordered-keymap,\n             any.foo -bar +baz,\n             any.ghc installed,\n             any.text ==2.1.2\n";
        assert_eq!(
            pins(text),
            [
                ("aeson".to_owned(), "2.2.3.0".to_owned()),
                ("text".to_owned(), "2.1.2".to_owned())
            ]
        );
    }

    #[test]
    fn a_pinned_package_with_a_known_advisory_is_reported() {
        let locked =
            parse_cabal_freeze("constraints: any.xml-conduit ==1.9.1.0\n").expect("parsed");
        let snapshot = r#"[{"id":"OSV-TEST-hackage","modified":"2026-09-18T00:00:00Z","affected":[{"package":{"ecosystem":"Hackage","name":"xml-conduit"},"ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.9.1.1"}]}]}]}]"#;
        let advisories = crate::vuln::parse_snapshot(snapshot).expect("snapshot");
        let (findings, _) = crate::vuln::match_packages(&locked, &advisories);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn missing_constraints_is_an_error() {
        let error = parse_cabal_freeze("index-state: x\n").expect_err("error");
        assert!(error.contains("missing constraints"), "{error}");
    }
}
