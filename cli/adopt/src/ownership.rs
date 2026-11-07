/// Who owns a file `dx init` writes, and what dx still touches after you edit it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// dx rewrites the whole file to match the running dx.
    Whole,
    /// The file extends a base dx owns; only the base is rewritten.
    Extends,
    /// dx sets its own keys; other keys, comments and order stay.
    Keys,
    /// dx rewrites only its marked block.
    Block,
    /// dx adds its missing lines and leaves the rest.
    Lines,
    /// dx updates only the rules_dx pin.
    Pin,
    /// Written when missing, never rewritten.
    Once,
}

impl Ownership {
    pub const ALL: [Ownership; 7] = [
        Ownership::Whole,
        Ownership::Extends,
        Ownership::Keys,
        Ownership::Block,
        Ownership::Lines,
        Ownership::Pin,
        Ownership::Once,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Ownership::Whole => "whole",
            Ownership::Extends => "extends",
            Ownership::Keys => "keys",
            Ownership::Block => "block",
            Ownership::Lines => "lines",
            Ownership::Pin => "pin",
            Ownership::Once => "once",
        }
    }

    /// The one-line comment that tells a reader what dx does to this file, in the file's comment
    /// syntax. `None` for formats that cannot carry a comment or where one would be noise.
    /// `edit_instead` names the file to edit for `whole`, or the base for `extends`.
    pub fn header(self, path: &str, edit_instead: Option<&str>) -> Option<String> {
        let prefix = comment_prefix(path)?;
        let text = match self {
            Ownership::Whole => match edit_instead {
                Some(file) => format!("dx: generated, do not edit; edit {file} instead"),
                None => "dx: generated, do not edit".to_owned(),
            },
            Ownership::Extends => match edit_instead {
                Some(base) => format!("dx: yours; dx owns only the base it extends, {base}"),
                None => "dx: yours; dx owns only the base it extends".to_owned(),
            },
            Ownership::Keys => "dx: yours; dx sets only its own keys".to_owned(),
            Ownership::Block => "dx: yours; dx rewrites only its marked block".to_owned(),
            Ownership::Lines => "dx: yours; dx adds its missing lines".to_owned(),
            Ownership::Pin => "dx: yours, except the rules_dx pin".to_owned(),
            Ownership::Once => "dx: written once, yours to edit".to_owned(),
        };
        Some(format!("{prefix}{text}"))
    }
}

const OVERRIDE_MARKER: &str = "dx: override of base ";

/// The base blob sha in a `dx: override of base <sha>` header, read from the leading comment
/// lines of a root config file.
pub fn override_base(text: &str) -> Option<&str> {
    let (header, _) = split_override_header(text);
    let (_, rest) = header.lines().last()?.split_once(OVERRIDE_MARKER)?;
    rest.split_whitespace().next()
}

/// Splits a root config file after its `dx: override of base <sha>` line: the leading lines up
/// to and including that line, then the rest. Without the header it is `("", text)`.
pub fn split_override_header(text: &str) -> (&str, &str) {
    let mut cut = 0;
    for line in text.split_inclusive('\n') {
        let start = line.trim_start_matches('\u{feff}').trim_start();
        let comment = start.is_empty()
            || ["#", "//", "REM", ";", "--"]
                .iter()
                .any(|p| start.starts_with(p));
        if !comment {
            break;
        }
        cut += line.len();
        if line.contains(OVERRIDE_MARKER) {
            return text.split_at(cut);
        }
    }
    ("", text)
}

fn comment_prefix(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    match extension {
        Some("json" | "md") => None,
        Some("cmd" | "bat") => Some("REM "),
        Some("yml" | "yaml" | "toml" | "bazel" | "bzl" | "bazelrc" | "sh" | "py") => Some("# "),
        _ => match name {
            ".envrc" | ".gitignore" | ".bazelignore" | ".gitattributes" | ".editorconfig" => {
                Some("# ")
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::Ownership;

    #[test]
    fn labels_are_distinct_lowercase_words() {
        let labels: Vec<&str> = Ownership::ALL.iter().map(|own| own.label()).collect();
        assert_eq!(
            labels,
            ["whole", "extends", "keys", "block", "lines", "pin", "once"]
        );
    }

    #[test]
    fn header_uses_the_comment_syntax_of_the_file() {
        assert_eq!(
            Ownership::Whole
                .header(".dx/bazelrc/x.bazelrc", None)
                .as_deref(),
            Some("# dx: generated, do not edit")
        );
        assert_eq!(
            Ownership::Whole
                .header("tools/dx.cmd", Some("MODULE.bazel"))
                .as_deref(),
            Some("REM dx: generated, do not edit; edit MODULE.bazel instead")
        );
        assert_eq!(
            Ownership::Extends
                .header("ruff.toml", Some(".dx/ruff.toml"))
                .as_deref(),
            Some("# dx: yours; dx owns only the base it extends, .dx/ruff.toml")
        );
        assert_eq!(
            Ownership::Pin
                .header(".github/workflows/ci.yml", None)
                .as_deref(),
            Some("# dx: yours, except the rules_dx pin")
        );
        assert_eq!(
            Ownership::Lines.header(".gitignore", None).as_deref(),
            Some("# dx: yours; dx adds its missing lines")
        );
        assert_eq!(
            Ownership::Once.header(".envrc", None).as_deref(),
            Some("# dx: written once, yours to edit")
        );
    }

    #[test]
    fn override_header_gives_the_base_sha() {
        assert_eq!(
            super::override_base("# dx: override of base abc123\nkey = 1\n"),
            Some("abc123")
        );
        assert_eq!(
            super::override_base("// dx: override of base f00 extra\n{}\n"),
            Some("f00")
        );
        assert_eq!(
            super::override_base("key = 1\n# dx: override of base abc\n"),
            None
        );
        assert_eq!(super::override_base("# yours\n"), None);
        assert_eq!(
            super::override_base("\u{feff}# dx: override of base abc\r\nkey = 1\r\n"),
            Some("abc")
        );
    }

    #[test]
    fn split_override_header_cuts_after_the_marker_line() {
        assert_eq!(
            super::split_override_header("# yours\n# dx: override of base abc\nkey = 1\n"),
            ("# yours\n# dx: override of base abc\n", "key = 1\n")
        );
        assert_eq!(
            super::split_override_header("key = 1\n# dx: override of base abc\n"),
            ("", "key = 1\n# dx: override of base abc\n")
        );
        assert_eq!(
            super::split_override_header("\u{feff}# dx: override of base abc\r\nkey = 1\r\n"),
            ("\u{feff}# dx: override of base abc\r\n", "key = 1\r\n")
        );
    }

    #[test]
    fn header_is_none_where_a_comment_does_not_fit() {
        for path in [
            ".vscode/settings.json",
            "AGENTS.md",
            ".bazelversion",
            ".dx/pins",
        ] {
            assert_eq!(Ownership::Once.header(path, None), None, "{path}");
        }
    }
}
