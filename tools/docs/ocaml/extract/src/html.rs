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
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
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
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn attributes(source: &str) -> Vec<(String, String)> {
    let bytes = source.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' && bytes[i] != b'/' {
            i += 1;
        }
        if start == i {
            break;
        }
        let name = source[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let from = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value = decode(&source[from..i]);
                i += 1;
            } else {
                let from = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = decode(&source[from..i]);
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
        let Some(open) = rest.find('<') else {
            append(&mut stack, text_node(rest));
            break;
        };
        if open > 0 {
            append(&mut stack, text_node(&rest[..open]));
        }
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(end) = rest.find('>') else {
            append(&mut stack, text_node(rest));
            break;
        };
        let inner = &rest[1..end];
        rest = &rest[end + 1..];
        if let Some(name) = inner.strip_prefix('/') {
            close(&mut stack, &name.trim().to_ascii_lowercase());
        } else if inner.starts_with('!') || inner.starts_with('?') {
            continue;
        } else {
            let split = inner
                .find(|c: char| c.is_ascii_whitespace() || c == '/')
                .unwrap_or(inner.len());
            let name = inner[..split].to_ascii_lowercase();
            let node = Node {
                attrs: attributes(&inner[split..]),
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
        name if name.len() == 2 && name.starts_with('h') && name.as_bytes()[1].is_ascii_digit() => {
            let level = usize::from(name.as_bytes()[1] - b'0').clamp(1, 6);
            out.push_str("\n\n");
            out.push_str(&"#".repeat(level));
            out.push(' ');
            out.push_str(&text(node, &["anchor"]));
            out.push_str("\n\n");
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
