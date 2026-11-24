#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Repo<'a> {
    Local,
    Apparent(&'a str),
    Canonical(&'a str),
}

impl std::fmt::Display for Repo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Repo::Local => Ok(()),
            Repo::Apparent(name) => write!(f, "@{name}"),
            Repo::Canonical(name) => write!(f, "@@{name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label<'a> {
    pub repo: Repo<'a>,
    pub package: &'a str,
    pub name: &'a str,
    pub relative: bool,
}

impl<'a> Label<'a> {
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

    #[must_use]
    pub fn repo_name(&self) -> Option<&'a str> {
        match self.repo {
            Repo::Local => None,
            Repo::Apparent(name) | Repo::Canonical(name) => Some(name),
        }
    }

    #[must_use]
    pub fn package_label(&self) -> String {
        format!("//{}", self.package)
    }

    #[must_use]
    pub fn file_path(&self) -> String {
        if self.package.is_empty() {
            self.name.to_owned()
        } else {
            format!("{}/{}", self.package, self.name)
        }
    }
}

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
    fn repos_spell_their_prefix() {
        let prefix = |text| Label::parse(text).map(|l| l.repo.to_string());
        assert_eq!(prefix("//p:n").as_deref(), Some(""));
        assert_eq!(prefix("@r//p:n").as_deref(), Some("@r"));
        assert_eq!(prefix("@//p:n").as_deref(), Some("@"));
        assert_eq!(prefix("@@r+//p:n").as_deref(), Some("@@r+"));
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
