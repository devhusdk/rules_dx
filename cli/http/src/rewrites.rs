use std::path::Path;

use regex::Regex;

use crate::Error;

#[derive(Clone, Debug, Default)]
pub struct Rewrites {
    rewrites: Vec<(Regex, Vec<String>)>,
    allow: Vec<String>,
    block: Vec<String>,
    all_blocked_message: Option<String>,
}

impl Rewrites {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        Self::parse(&text).map_err(|error| Error::Config(format!("{}: {error}", path.display())))
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut rewrites = Self::default();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |reason: &str| Error::Config(format!("line {}: {reason}: {line}", index + 1));
            let mut words = line.split_whitespace();
            let directive = words.next().unwrap_or_default();
            let args: Vec<&str> = words.collect();
            match (directive, args.as_slice()) {
                ("allow", [host]) => rewrites.allow.push((*host).to_owned()),
                ("block", [host]) => rewrites.block.push((*host).to_owned()),
                ("rewrite", [pattern, replacement]) => {
                    let regex = Regex::new(&format!("^(?:{pattern})$"))
                        .map_err(|error| bad(&error.to_string()))?;
                    rewrites.add(regex, java_replacement(replacement));
                }
                ("all_blocked_message", [_, ..]) => {
                    let message = line.strip_prefix(directive).unwrap_or_default().trim();
                    rewrites.all_blocked_message = Some(message.to_owned());
                }
                ("allow" | "block" | "rewrite" | "all_blocked_message", _) => {
                    return Err(bad("wrong number of arguments"));
                }
                _ => return Err(bad("unknown directive")),
            }
        }
        Ok(rewrites)
    }

    fn add(&mut self, regex: Regex, replacement: String) {
        let same = self
            .rewrites
            .iter_mut()
            .find(|(known, _)| known.as_str() == regex.as_str());
        match same {
            Some((_, replacements)) if replacements.contains(&replacement) => {}
            Some((_, replacements)) => replacements.push(replacement),
            None => self.rewrites.push((regex, vec![replacement])),
        }
    }

    pub fn amend(&self, url: &str) -> Result<Vec<String>, Error> {
        let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
        let mut urls: Vec<String> = Vec::new();
        for (regex, replacements) in &self.rewrites {
            let Some(captures) = regex.captures(rest) else {
                continue;
            };
            for replacement in replacements {
                let mut rewritten = String::new();
                captures.expand(replacement, &mut rewritten);
                let full = if rewritten.contains("://") || scheme.is_empty() {
                    rewritten
                } else {
                    format!("{scheme}://{rewritten}")
                };
                if !urls.contains(&full) {
                    urls.push(full);
                }
            }
        }
        if urls.is_empty() {
            urls.push(url.to_owned());
        }
        urls.retain(|candidate| !self.blocked(candidate));
        if urls.is_empty() {
            let mut message = format!("Configured URL rewriter blocked all URLs: [{url}]");
            if let Some(extra) = &self.all_blocked_message {
                message.push_str(" - ");
                message.push_str(extra);
            }
            return Err(Error::Blocked(message));
        }
        Ok(urls)
    }

    fn blocked(&self, url: &str) -> bool {
        let host = host(url);
        let matches = |name: &String| host == name || host.ends_with(&format!(".{name}"));
        !self.allow.iter().any(matches)
            && self.block.iter().any(|name| name == "*" || matches(name))
    }
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if let Some(bracketed) = authority.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or_default();
    }
    authority.split(':').next().unwrap_or_default()
}

fn java_replacement(replacement: &str) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('$') => out.push_str("$$"),
                Some(next) => out.push(next),
                None => out.push('\\'),
            },
            '$' => {
                let mut group = String::new();
                while let Some(digit) = chars.next_if(char::is_ascii_digit) {
                    group.push(digit);
                }
                if group.is_empty() {
                    out.push_str("$$");
                } else {
                    out.push_str("${");
                    out.push_str(&group);
                    out.push('}');
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Rewrites {
        Rewrites::parse(text).expect("config")
    }

    #[test]
    fn the_repo_config_tries_the_lua_mirror_then_upstream() {
        let rewrites = parsed(include_str!("../../../tools/workspace/downloader.cfg"));
        assert_eq!(
            rewrites.amend("https://www.lua.org/ftp/lua-5.4.7.tar.gz"),
            Ok(vec![
                "https://bcr.cloudflaremirrors.com/www.lua.org/ftp/lua-5.4.7.tar.gz".to_owned(),
                "https://www.lua.org/ftp/lua-5.4.7.tar.gz".to_owned(),
            ])
        );
        assert_eq!(
            rewrites.amend("https://github.com/a/b.tar.gz"),
            Ok(vec!["https://github.com/a/b.tar.gz".to_owned()])
        );
    }

    #[test]
    fn a_blocked_host_is_rewritten_to_its_mirror_and_subdomains_stay_blocked() {
        let rewrites = parsed(
            "all_blocked_message See go/mirror\n\
             block example.com\n\
             rewrite example.com/(.*) mirror.corp.internal/example.com/$1\n",
        );
        assert_eq!(
            rewrites.amend("https://example.com/foo/bar.tar.gz"),
            Ok(vec![
                "https://mirror.corp.internal/example.com/foo/bar.tar.gz".to_owned()
            ])
        );
        assert_eq!(
            rewrites.amend("https://sub.example.com/x"),
            Err(Error::Blocked(
                "Configured URL rewriter blocked all URLs: [https://sub.example.com/x] - See go/mirror"
                    .to_owned()
            ))
        );
    }

    #[test]
    fn each_pattern_yields_its_urls_in_the_order_it_first_appears() {
        let rewrites = parsed(
            "rewrite (.*)/a/(.*) one/$2\n\
             rewrite host/(.*) two/$1\n\
             rewrite (.*)/a/(.*) three/$1\n",
        );
        assert_eq!(
            rewrites.amend("http://host/a/b"),
            Ok(vec![
                "http://one/b".to_owned(),
                "http://three/host".to_owned(),
                "http://two/a/b".to_owned(),
            ])
        );
    }

    #[test]
    fn allow_wins_over_block() {
        let rewrites = parsed("block *\nallow mirror.example\n");
        assert_eq!(
            rewrites.amend("https://cdn.mirror.example:8443/x"),
            Ok(vec!["https://cdn.mirror.example:8443/x".to_owned()])
        );
        assert!(rewrites.amend("https://elsewhere.example/x").is_err());
    }

    #[test]
    fn a_replacement_with_a_scheme_keeps_it() {
        let rewrites = parsed("rewrite up\\.example/(.*) http://127.0.0.1:8080/$1\n");
        assert_eq!(
            rewrites.amend("https://up.example/a.zip"),
            Ok(vec!["http://127.0.0.1:8080/a.zip".to_owned()])
        );
    }

    #[test]
    fn java_group_references_are_kept_apart_from_following_text() {
        let rewrites = parsed("rewrite (a)(b) $2x$1\\$\n");
        assert_eq!(
            rewrites.amend("https://ab"),
            Ok(vec!["https://bxa$".to_owned()])
        );
    }

    #[test]
    fn bad_lines_are_refused() {
        for text in [
            "mirror x\n",
            "rewrite only-one\n",
            "block\n",
            "rewrite ( x\n",
        ] {
            assert!(
                matches!(Rewrites::parse(text), Err(Error::Config(_))),
                "{text}"
            );
        }
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let rewrites = parsed("# note\n\n   \nblock a.example\n");
        assert!(rewrites.amend("https://a.example/x").is_err());
    }
}
