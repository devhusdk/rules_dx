//! Bazel label spelling: one parser for `@repo//package:name` and its short forms.
//!
//! The parser is structural only. It splits a label into repository, package and
//! name and does not check the characters inside each part, so target patterns
//! such as `@hub//...` or `//pkg:all` parse too, with the wildcard kept as the
//! package or name.

/// The repository part of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Repo<'a> {
    /// No `@`: the repository the label is read in (`//p:n`, `:n`).
    Local,
    /// `@name`, as the reading module spells it. `@//p` is the main repository.
    Apparent(&'a str),
    /// `@@name`, the repository name Bazel assigns (`rules_go+`, `+ext+hub`).
    Canonical(&'a str),
}

/// One parsed label. The parts borrow from the text it was parsed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label<'a> {
    pub repo: Repo<'a>,
    /// The package path, `""` for the root package. Unknown for a
    /// package-relative label, where it is also `""`; see [`Label::relative`].
    pub package: &'a str,
    pub name: &'a str,
    /// Whether the label is package-relative (`:n`), so `package` is the reader's.
    pub relative: bool,
}

impl<'a> Label<'a> {
    /// Parses `//p`, `//p:n`, `:n`, `@r`, `@r//p:n`, `@@r+//p:n` and `@//p:n`.
    ///
    /// Returns `None` for text that is not label-shaped: no leading `//`, `:` or
    /// `@`, an empty name, or a repository not followed by `//`.
    #[must_use]
    pub fn parse(text: &'a str) -> Option<Self> {
        if let Some(name) = text.strip_prefix(':') {
            return (!name.is_empty()).then_some(Label {
                repo: Repo::Local,
                package: "",
                name,
                relative: true,
            });
        }
        let (repo, rest) = match text.strip_prefix('@') {
            Some(after) => {
                let (canonical, after) = match after.strip_prefix('@') {
                    Some(after) => (true, after),
                    None => (false, after),
                };
                let (name, rest) = match after.split_once("//") {
                    Some((name, rest)) => (name, Some(rest)),
                    None => (after, None),
                };
                let repo = if canonical {
                    Repo::Canonical(name)
                } else {
                    Repo::Apparent(name)
                };
                match rest {
                    Some(rest) => (repo, rest),
                    // `@r` is shorthand for `@r//:r`.
                    None if name.is_empty() || name.contains([':', '/']) => return None,
                    None => {
                        return Some(Label {
                            repo,
                            package: "",
                            name,
                            relative: false,
                        });
                    }
                }
            }
            None => (Repo::Local, text.strip_prefix("//")?),
        };
        let (package, name) = match rest.split_once(':') {
            Some((package, name)) => (package, name),
            // `//a/b` is shorthand for `//a/b:b`.
            None => (rest, rest.rsplit('/').next().unwrap_or(rest)),
        };
        if name.is_empty() {
            return None;
        }
        Some(Label {
            repo,
            package,
            name,
            relative: false,
        })
    }

    /// The repository name after `@` or `@@`, or `None` when the label has no `@`.
    #[must_use]
    pub fn repo_name(&self) -> Option<&'a str> {
        match self.repo {
            Repo::Local => None,
            Repo::Apparent(name) | Repo::Canonical(name) => Some(name),
        }
    }

    /// The package spelled without its repository, as `//package`.
    #[must_use]
    pub fn package_label(&self) -> String {
        format!("//{}", self.package)
    }

    /// The workspace-relative path of a source file this label names.
    #[must_use]
    pub fn file_path(&self) -> String {
        if self.package.is_empty() {
            self.name.to_owned()
        } else {
            format!("{}/{}", self.package, self.name)
        }
    }
}

/// The repository a label names after `@` or `@@`, or `None` for a label
/// without one or text that is not a label.
#[must_use]
pub fn repo_of(text: &str) -> Option<&str> {
    Label::parse(text)?.repo_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label<'a>(repo: Repo<'a>, package: &'a str, name: &'a str) -> Label<'a> {
        Label {
            repo,
            package,
            name,
            relative: false,
        }
    }

    #[test]
    fn main_repo_package_names_its_last_segment() {
        assert_eq!(Label::parse("//p"), Some(label(Repo::Local, "p", "p")));
        assert_eq!(Label::parse("//a/b"), Some(label(Repo::Local, "a/b", "b")));
        assert_eq!(Label::parse("//:n"), Some(label(Repo::Local, "", "n")));
        assert_eq!(
            Label::parse("//a/b:c/d.txt"),
            Some(label(Repo::Local, "a/b", "c/d.txt"))
        );
    }

    #[test]
    fn bare_repo_names_its_own_root_target() {
        assert_eq!(
            Label::parse("@r"),
            Some(label(Repo::Apparent("r"), "", "r"))
        );
        assert_eq!(
            Label::parse("@@r+"),
            Some(label(Repo::Canonical("r+"), "", "r+"))
        );
    }

    #[test]
    fn apparent_and_canonical_repos() {
        assert_eq!(
            Label::parse("@r//p:n"),
            Some(label(Repo::Apparent("r"), "p", "n"))
        );
        assert_eq!(
            Label::parse("@r//p"),
            Some(label(Repo::Apparent("r"), "p", "p"))
        );
        assert_eq!(
            Label::parse("@@r+//p:n"),
            Some(label(Repo::Canonical("r+"), "p", "n"))
        );
        assert_eq!(
            Label::parse("@@+ext+hub//:n"),
            Some(label(Repo::Canonical("+ext+hub"), "", "n"))
        );
        assert_eq!(
            Label::parse("@//p:n"),
            Some(label(Repo::Apparent(""), "p", "n"))
        );
        assert_eq!(
            Label::parse("@@//p:n"),
            Some(label(Repo::Canonical(""), "p", "n"))
        );
        assert_eq!(
            Label::parse("@r//p:n").and_then(|l| l.repo_name()),
            Some("r")
        );
        assert_eq!(
            Label::parse("@@r+//p:n").and_then(|l| l.repo_name()),
            Some("r+")
        );
        assert_eq!(Label::parse("//p:n").and_then(|l| l.repo_name()), None);
    }

    #[test]
    fn package_relative_name() {
        assert_eq!(
            Label::parse(":n"),
            Some(Label {
                repo: Repo::Local,
                package: "",
                name: "n",
                relative: true,
            })
        );
    }

    #[test]
    fn target_patterns_keep_their_wildcard() {
        assert_eq!(
            Label::parse("@hub//:all"),
            Some(label(Repo::Apparent("hub"), "", "all"))
        );
        assert_eq!(
            Label::parse("//p/..."),
            Some(label(Repo::Local, "p/...", "..."))
        );
    }

    #[test]
    fn non_labels_are_refused() {
        for text in [
            "",
            "p:n",
            "p",
            ":",
            "//p:",
            "@",
            "@@",
            "@r:n",
            "@r/p",
            "https://x",
        ] {
            assert_eq!(Label::parse(text), None, "{text}");
        }
    }

    #[test]
    fn file_paths() {
        let path = |text| Label::parse(text).map(|l| l.file_path());
        assert_eq!(path("//:BUILD.bazel").as_deref(), Some("BUILD.bazel"));
        assert_eq!(path("//a/b:c.txt").as_deref(), Some("a/b/c.txt"));
        assert_eq!(
            Label::parse("//a/b:c")
                .map(|l| l.package_label())
                .as_deref(),
            Some("//a/b")
        );
    }
}
