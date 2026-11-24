pub(crate) struct Tag<'a> {
    pub name: &'a str,
    pub attrs: &'a str,
    pub empty: bool,
    pub tail: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Unterminated(pub &'static str);

const SKIPPED: [(&str, &str, &str); 3] = [
    ("!--", "-->", "comment"),
    ("![CDATA[", "]]>", "CDATA section"),
    ("?", "?>", "processing instruction"),
];

pub(crate) fn tags(xml: &str) -> Tags<'_> {
    Tags { rest: xml }
}

pub(crate) struct Tags<'a> {
    rest: &'a str,
}

impl<'a> Iterator for Tags<'a> {
    type Item = Result<Tag<'a>, Unterminated>;

    fn next(&mut self) -> Option<Self::Item> {
        'markup: loop {
            let (_, after) = self.rest.split_once('<')?;
            for (open, close, what) in SKIPPED {
                if let Some(body) = after.strip_prefix(open) {
                    let Some((_, tail)) = body.split_once(close) else {
                        self.rest = "";
                        return Some(Err(Unterminated(what)));
                    };
                    self.rest = tail;
                    continue 'markup;
                }
            }
            let Some((tag, tail)) = split_at_tag_end(after) else {
                self.rest = "";
                return Some(Err(Unterminated("tag")));
            };
            self.rest = tail;
            let empty = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let (name, attrs) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
            return Some(Ok(Tag {
                name,
                attrs,
                empty,
                tail,
            }));
        }
    }
}

fn split_at_tag_end(text: &str) -> Option<(&str, &str)> {
    let mut quote = None;
    for (at, ch) in text.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(open), _) if open == ch => quote = None,
            (None, '>') => {
                let (tag, end) = text.split_at_checked(at)?;
                return Some((tag, end.strip_prefix('>')?));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn attribute(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some((name, tail)) = rest.split_once('=') {
        let name = name.trim();
        let mut quoted = tail.trim_start().chars();
        let quote = quoted.next().filter(|c| *c == '"' || *c == '\'')?;
        let (value, after) = quoted.as_str().split_once(quote)?;
        if name == key {
            return Some(unescape(value));
        }
        rest = after;
    }
    None
}

pub(crate) fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((head, tail)) = rest.split_once('&') {
        out.push_str(head);
        let decoded = tail
            .split_once(';')
            .and_then(|(name, after)| entity(name).map(|ch| (ch, after)));
        if let Some((ch, after)) = decoded {
            out.push(ch);
            rest = after;
        } else {
            out.push('&');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "amp" => Some('&'),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
#[path = "xml_tests.rs"]
mod xml_tests;
