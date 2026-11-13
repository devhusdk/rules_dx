//! Prints the changed files on stdin that start with a prefix in `.github/review-paths.txt`.

use std::io::{BufRead, Write};

const DEFAULT_LIST: &str = ".github/review-paths.txt";

fn prefixes(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .collect()
}

/// Reads paths from stdin and prints those that start with a prefix from the list file.
pub fn filter(args: &[String]) -> Result<(), String> {
    let list = args.first().map_or(DEFAULT_LIST, String::as_str);
    let text = std::fs::read_to_string(list).map_err(|error| format!("read {list}: {error}"))?;
    let prefixes = prefixes(&text);
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let file = line.map_err(|error| error.to_string())?;
        if prefixes.iter().any(|prefix| file.starts_with(prefix)) {
            writeln!(out, "{file}").map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prefixes;

    #[test]
    fn skips_comments_and_blank_lines() {
        assert_eq!(
            prefixes("# note\n\ncli/worktree/\n  # indented\ncore/\n"),
            ["cli/worktree/", "core/"]
        );
    }
}
