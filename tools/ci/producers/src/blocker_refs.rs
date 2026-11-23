use std::fmt::Write as _;
use std::io::Read;

fn name_len(text: &str) -> usize {
    text.bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        .count()
}

fn after(text: &str, at: usize) -> &str {
    text.get(at..).unwrap_or_default()
}

fn digits_len(text: &str) -> usize {
    text.bytes().take_while(u8::is_ascii_digit).count()
}

fn url_len(text: &str) -> Option<usize> {
    let mut rest = text.strip_prefix("https://github.com/")?;
    let mut used = text.len() - rest.len();
    for _ in 0..2 {
        let name = name_len(rest);
        rest = after(rest, name).strip_prefix('/').filter(|_| name > 0)?;
        used += name + 1;
    }
    let kind = ["pull/", "issues/"]
        .into_iter()
        .find(|kind| rest.starts_with(kind))?;
    let digits = digits_len(after(rest, kind.len()));
    (digits > 0).then_some(used + kind.len() + digits)
}

fn short_len(text: &str) -> Option<usize> {
    let owner = name_len(text);
    let rest = after(text, owner).strip_prefix('/').filter(|_| owner > 0)?;
    let repo = name_len(rest);
    let rest = after(rest, repo).strip_prefix('#').filter(|_| repo > 0)?;
    let digits = digits_len(rest);
    (digits > 0).then_some(owner + 1 + repo + 1 + digits)
}

fn bare_len(text: &str) -> Option<usize> {
    let digits = digits_len(text.strip_prefix('#')?);
    (digits > 0).then_some(1 + digits)
}

fn take(text: &str, matcher: fn(&str) -> Option<usize>) -> (Vec<String>, String) {
    let mut found = Vec::new();
    let mut left = String::new();
    let mut at = 0;
    while at < text.len() {
        if let Some(len) = matcher(after(text, at)) {
            found.push(text.get(at..at + len).unwrap_or_default().to_string());
            at += len;
            continue;
        }
        let step = after(text, at).chars().next().map_or(1, char::len_utf8);
        left.push_str(text.get(at..at + step).unwrap_or_default());
        at += step;
    }
    (found, left)
}

pub fn refs(local_repo: &str, line: &str) -> String {
    let (urls, rest) = take(line, url_len);
    let (shorts, rest) = take(&rest, short_len);
    let (bares, rest) = take(&rest, bare_len);
    let release = rest
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|word| word.eq_ignore_ascii_case("release"));
    let column = |repo: &str| {
        if release && !repo.eq_ignore_ascii_case(local_repo) {
            " release"
        } else {
            ""
        }
    };
    let mut out = String::new();
    for url in urls {
        let path = url.trim_start_matches("https://github.com/");
        let mut parts = path.split('/');
        let owner = parts.next().unwrap_or_default();
        let repo = parts.next().unwrap_or_default();
        let number = parts.next_back().unwrap_or_default();
        let column = column(&format!("{owner}/{repo}"));
        writeln!(out, "{owner}/{repo} {number} {url}{column}").unwrap_or_default();
    }
    for short in shorts {
        let (repo, number) = short.split_once('#').unwrap_or_default();
        writeln!(out, "{repo} {number} {short}{}", column(repo)).unwrap_or_default();
    }
    for bare in bares {
        writeln!(out, "{local_repo} {} {bare}", bare.strip_prefix('#').unwrap_or_default()).unwrap_or_default();
    }
    out
}

pub fn print(args: &[String]) -> Result<(), String> {
    let [local_repo] = args else {
        return Err("usage: producers_bin --blocker-refs OWNER/REPO".to_string());
    };
    let mut line = String::new();
    std::io::stdin()
        .read_to_string(&mut line)
        .map_err(|error| error.to_string())?;
    print!("{}", refs(local_repo, &line));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::refs;

    const LOCAL: &str = "devhusdk/rules_dx";

    #[test]
    fn bare_numbers_are_local() {
        assert_eq!(
            refs(LOCAL, "Blocked on: #4 and #5\n"),
            "devhusdk/rules_dx 4 #4\ndevhusdk/rules_dx 5 #5\n"
        );
    }

    #[test]
    fn owner_repo_refs_are_not_local() {
        assert_eq!(
            refs(LOCAL, "Blocked on: bazelverse/rules_elixir#2\n"),
            "bazelverse/rules_elixir 2 bazelverse/rules_elixir#2\n"
        );
    }

    #[test]
    fn bare_number_after_a_cross_repo_ref_stays_local() {
        assert_eq!(
            refs(LOCAL, "Blocked on: bazelverse/rules_elixir#2, #3\n"),
            "bazelverse/rules_elixir 2 bazelverse/rules_elixir#2\ndevhusdk/rules_dx 3 #3\n"
        );
    }

    #[test]
    fn urls_name_their_own_repo() {
        assert_eq!(
            refs(
                LOCAL,
                "Blocked on: https://github.com/devhusdk/rules_dx/pull/9 https://github.com/apache/avro/pull/4076 https://github.com/a/b/issues/7\n"
            ),
            "devhusdk/rules_dx 9 https://github.com/devhusdk/rules_dx/pull/9\n\
             apache/avro 4076 https://github.com/apache/avro/pull/4076\n\
             a/b 7 https://github.com/a/b/issues/7\n"
        );
    }

    #[test]
    fn owner_action_has_no_refs() {
        assert_eq!(refs(LOCAL, "Blocked on: owner cuts the release\n"), "");
    }

    #[test]
    fn malformed_refs_are_skipped() {
        assert_eq!(refs(LOCAL, "https://github.com/a/b/pull/ a/b# x/y# /y"), "");
        assert_eq!(refs(LOCAL, "# 1 é #2"), "devhusdk/rules_dx 2 #2\n");
    }

    #[test]
    fn release_lines_mark_foreign_refs() {
        assert_eq!(
            refs(
                LOCAL,
                "Blocked on: https://github.com/bazelbuild/rules_rust/pull/4380 (merged and in a rules_rust release)\n"
            ),
            "bazelbuild/rules_rust 4380 https://github.com/bazelbuild/rules_rust/pull/4380 release\n"
        );
        assert_eq!(refs(LOCAL, "Blocked on: a/b#7, Release\n"), "a/b 7 a/b#7 release\n");
    }

    #[test]
    fn release_lines_leave_local_refs_unmarked() {
        assert_eq!(
            refs(LOCAL, "Blocked on: #4, devhusdk/rules_dx#5, DevHusdk/Rules_DX#6, a/b#7 in a release\n"),
            "devhusdk/rules_dx 5 devhusdk/rules_dx#5\n\
             DevHusdk/Rules_DX 6 DevHusdk/Rules_DX#6\n\
             a/b 7 a/b#7 release\n\
             devhusdk/rules_dx 4 #4\n"
        );
    }

    #[test]
    fn release_must_be_a_word_outside_the_refs() {
        assert_eq!(
            refs(LOCAL, "Blocked on: https://github.com/googleapis/release-please/pull/5\n"),
            "googleapis/release-please 5 https://github.com/googleapis/release-please/pull/5\n"
        );
        assert_eq!(refs(LOCAL, "Blocked on: a/b#1 released\n"), "a/b 1 a/b#1\n");
    }

    #[test]
    fn print_needs_one_repo_argument() {
        assert!(super::print(&[]).is_err());
    }
}
