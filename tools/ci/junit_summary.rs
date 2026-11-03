//! Turns the failing cases of a `junit.xml` report into a Markdown table.

const MAX_ROWS: usize = 50;

struct Failure {
    target: String,
    case: String,
    message: String,
}

fn unescape(text: &str) -> String {
    text.replace("&#10;", "\n")
        .replace("&#13;", "")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(unescape(&tag[start..end]))
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn failures(xml: &str) -> Vec<Failure> {
    let mut found = Vec::new();
    let mut target = String::new();
    let mut case = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        let after = &rest[end + 1..];
        let name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        match name {
            "testsuite" => target = attr(tag, "name").unwrap_or_default(),
            "testcase" => case = attr(tag, "name").unwrap_or_default(),
            "failure" | "error" => {
                let mut message = first_line(&attr(tag, "message").unwrap_or_default());
                if message.is_empty() && !tag.ends_with('/') {
                    let body = after.split('<').next().unwrap_or_default();
                    message = first_line(&unescape(body));
                }
                found.push(Failure {
                    target: target.clone(),
                    case: case.clone(),
                    message,
                });
            }
            _ => {}
        }
        rest = after;
    }
    found
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Returns the Markdown table of failing cases, or an empty string when none failed.
pub fn render(xml: &str) -> String {
    let all = failures(xml);
    if all.is_empty() {
        return String::new();
    }
    let mut out =
        String::from("### Failing tests\n\n| Target | Case | Message |\n| --- | --- | --- |\n");
    for failure in all.iter().take(MAX_ROWS) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            cell(&failure.target),
            cell(&failure.case),
            cell(&failure.message)
        ));
    }
    if all.len() > MAX_ROWS {
        out.push_str(&format!("\n{} more failed.\n", all.len() - MAX_ROWS));
    }
    out
}
