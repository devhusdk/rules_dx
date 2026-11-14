//! Converts the `.lst` files a D program built with `-cov` writes into LCOV.

/// Returns the LCOV record for one `.lst` file, or None when the file has no code.
///
/// Each line is `count|source`; the count is blank for lines without code. The last
/// line names the source file: `<path> is 80% covered` or `<path> has no code`.
pub fn lst_to_lcov(text: &str) -> Option<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    let summary = lines.pop()?;
    let path = summary
        .rsplit_once(" is ")
        .or_else(|| summary.rsplit_once(" has no code"))
        .map(|(path, _)| path)?;
    let mut out = format!("SF:{path}\n");
    let (mut found, mut hit) = (0, 0);
    for (index, line) in lines.iter().enumerate() {
        let Some((count, _)) = line.split_once('|') else {
            continue;
        };
        let Ok(count) = count.trim().parse::<u64>() else {
            continue;
        };
        found += 1;
        hit += usize::from(count > 0);
        out.push_str(&format!("DA:{},{count}\n", index + 1));
    }
    if found == 0 {
        return None;
    }
    out.push_str(&format!("LF:{found}\nLH:{hit}\nend_of_record\n"));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_executable_lines_only() {
        let lst = "       |module greet;\n       |\n0000002|string hi() { return \"x\"; }\n0000000|void dead() {}\ngreet.d is 50% covered\n";
        assert_eq!(
            lst_to_lcov(lst).as_deref(),
            Some("SF:greet.d\nDA:3,2\nDA:4,0\nLF:2\nLH:1\nend_of_record\n")
        );
    }

    #[test]
    fn source_text_with_pipes_and_is_does_not_confuse_the_parse() {
        let lst = "0000001|auto x = a | b; // x is y\nsrc/a.d is 100% covered\n";
        let lcov = lst_to_lcov(lst);
        assert_eq!(
            lcov.as_deref(),
            Some("SF:src/a.d\nDA:1,1\nLF:1\nLH:1\nend_of_record\n")
        );
    }

    #[test]
    fn a_file_without_code_has_no_record() {
        assert_eq!(lst_to_lcov("       |module m;\nm.d has no code\n"), None);
    }

    #[test]
    fn a_file_without_a_summary_has_no_record() {
        assert_eq!(lst_to_lcov(""), None);
        assert_eq!(lst_to_lcov("0000001|x\n"), None);
    }
}
