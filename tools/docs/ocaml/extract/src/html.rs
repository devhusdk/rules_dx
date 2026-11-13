//! A small tolerant HTML reader for the fragments odoc writes.

/// One element, or a text node when `name` is empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

const VOID: [&str; 7] = ["br", "hr", "img", "meta", "link", "input", "wbr"];

impl Node {
    /// Wraps top-level nodes in an anonymous element.
    pub fn root(children: Vec<Node>) -> Node {
        Node {
            children,
            ..Node::default()
        }
    }

    /// Returns the value of one attribute.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Whether the class attribute lists `class`.
    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|list| list.split_whitespace().any(|c| c == class))
    }
}

/// Decodes the character references odoc emits.
pub fn decode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((before, after)) = rest.split_once('&') {
        out.push_str(before);
        let Some((entity, tail)) = after
            .split_once(';')
            .filter(|(entity, _)| entity.len() < 10)
        else {
            out.push('&');
            rest = after;
            continue;
        };
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse().ok(),
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = tail;
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn split_while(source: &str, keep: impl Fn(char) -> bool) -> (&str, &str) {
    let end = source.find(|c: char| !keep(c)).unwrap_or(source.len());
    source.split_at_checked(end).unwrap_or((source, ""))
}

fn attributes(source: &str) -> Vec<(String, String)> {
    let blank = |c: char| c.is_ascii_whitespace();
    let mut attrs = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| blank(c) || c == '/');
        let (name, after) = split_while(rest, |c| !blank(c) && c != '=' && c != '/');
        if name.is_empty() {
            break;
        }
        let name = name.to_ascii_lowercase();
        rest = after.trim_start_matches(blank);
        let mut value = String::new();
        if let Some(after_equals) = rest.strip_prefix('=') {
            let after_equals = after_equals.trim_start_matches(blank);
            let quoted = after_equals
                .strip_prefix('"')
                .map(|body| ('"', body))
                .or_else(|| after_equals.strip_prefix('\'').map(|body| ('\'', body)));
            if let Some((quote, body)) = quoted {
                let (raw, tail) = split_while(body, |c| c != quote);
                value = decode(raw);
                rest = tail.strip_prefix(quote).unwrap_or(tail);
            } else {
                let (raw, tail) = split_while(after_equals, |c| !blank(c));
                value = decode(raw);
                rest = tail;
            }
        }
        attrs.push((name, value));
    }
    attrs
}

fn append(stack: &mut [Node], node: Node) {
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
    }
}

fn close(stack: &mut Vec<Node>, name: &str) {
    if !stack.iter().skip(1).any(|n| n.name == name) {
        return;
    }
    while stack.len() > 1 {
        let Some(done) = stack.pop() else { break };
        let matched = done.name == name;
        append(stack, done);
        if matched {
            break;
        }
    }
}

/// Parses a fragment into its top-level nodes.
pub fn parse(source: &str) -> Vec<Node> {
    let mut stack = vec![Node::default()];
    let mut rest = source;
    while !rest.is_empty() {
        let Some((before, after)) = rest.split_once('<') else {
            append(&mut stack, text_node(rest));
            break;
        };
        if !before.is_empty() {
            append(&mut stack, text_node(before));
        }
        if let Some(comment) = after.strip_prefix("!--") {
            rest = comment.split_once("-->").map_or("", |(_, tail)| tail);
            continue;
        }
        let Some((inner, tail)) = after.split_once('>') else {
            append(&mut stack, text_node(rest));
            break;
        };
        rest = tail;
        if let Some(name) = inner.strip_prefix('/') {
            close(&mut stack, &name.trim().to_ascii_lowercase());
        } else if inner.starts_with('!') || inner.starts_with('?') {
            continue;
        } else {
            let (head, attribute_source) =
                split_while(inner, |c| !c.is_ascii_whitespace() && c != '/');
            let name = head.to_ascii_lowercase();
            let node = Node {
                attrs: attributes(attribute_source),
                name: name.clone(),
                ..Node::default()
            };
            if VOID.contains(&name.as_str()) || inner.ends_with('/') {
                append(&mut stack, node);
            } else {
                stack.push(node);
            }
        }
    }
    while stack.len() > 1 {
        let Some(done) = stack.pop() else { break };
        append(&mut stack, done);
    }
    stack.pop().map(|n| n.children).unwrap_or_default()
}

fn text_node(raw: &str) -> Node {
    Node {
        text: decode(raw),
        ..Node::default()
    }
}

fn gather(node: &Node, skip: &[&str], out: &mut String) {
    if node.name.is_empty() {
        out.push_str(&node.text);
        for child in &node.children {
            gather(child, skip, out);
        }
        return;
    }
    if skip.iter().any(|class| node.has_class(class)) {
        return;
    }
    let block = matches!(node.name.as_str(), "li" | "ol" | "ul" | "div" | "p" | "br");
    if block {
        out.push(' ');
    }
    for child in &node.children {
        gather(child, skip, out);
    }
    if block {
        out.push(' ');
    }
}

/// Returns the text of an element on one line, without the elements that carry a skipped class.
pub fn text(node: &Node, skip: &[&str]) -> String {
    let mut out = String::new();
    gather(node, skip, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn heading_level(name: &str) -> Option<usize> {
    let [digit] = name.strip_prefix('h')?.as_bytes() else {
        return None;
    };
    digit
        .is_ascii_digit()
        .then(|| usize::from(digit - b'0').clamp(1, 6))
}

fn render(node: &Node, out: &mut String) {
    if node.name.is_empty() {
        out.push_str(&node.text);
        for child in &node.children {
            render(child, out);
        }
        return;
    }
    if node.has_class("comment-delim") || node.has_class("anchor") {
        return;
    }
    let inline = |out: &mut String, mark: &str| {
        out.push_str(mark);
        for child in &node.children {
            render(child, out);
        }
        out.push_str(mark);
    };
    match node.name.as_str() {
        "p" | "div" => {
            out.push_str("\n\n");
            for child in &node.children {
                render(child, out);
            }
            out.push_str("\n\n");
        }
        "br" => out.push('\n'),
        "code" | "tt" => {
            let body = text(node, &[]);
            out.push('`');
            out.push_str(&body);
            out.push('`');
        }
        "pre" => {
            let mut body = String::new();
            gather(node, &[], &mut body);
            out.push_str("\n\n```ocaml\n");
            out.push_str(body.trim_matches('\n'));
            out.push_str("\n```\n\n");
        }
        "em" | "i" => inline(out, "*"),
        "strong" | "b" => inline(out, "**"),
        "ul" | "ol" => {
            out.push_str("\n\n");
            for child in node.children.iter().filter(|c| c.name == "li") {
                out.push_str("- ");
                let mut item = String::new();
                render(child, &mut item);
                out.push_str(&tidy(&item).replace('\n', "\n  "));
                out.push('\n');
            }
            out.push('\n');
        }
        "a" => {
            let label = text(node, &[]);
            match node.attr("href") {
                Some(href) if href.starts_with("http://") || href.starts_with("https://") => {
                    out.push_str(&format!("[{label}]({href})"));
                }
                _ => out.push_str(&label),
            }
        }
        name if heading_level(name).is_some() => {
            if let Some(level) = heading_level(name) {
                out.push_str("\n\n");
                out.push_str(&"#".repeat(level));
                out.push(' ');
                out.push_str(&text(node, &["anchor"]));
                out.push_str("\n\n");
            }
        }
        _ => {
            for child in &node.children {
                render(child, out);
            }
        }
    }
}

fn tidy(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in raw.replace("\r\n", "\n").lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        }
        let line = if fenced { line.trim_end() } else { line.trim() };
        if line.is_empty() && lines.last().is_some_and(String::is_empty) {
            continue;
        }
        lines.push(line.to_owned());
    }
    lines.join("\n").trim().to_owned()
}

/// Renders an element as markdown.
pub fn markdown(node: &Node) -> String {
    let mut out = String::new();
    render(node, &mut out);
    tidy(&out)
}
