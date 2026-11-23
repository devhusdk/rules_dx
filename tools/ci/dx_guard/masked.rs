pub struct Masked {
    pub code: String,
    pub mask: String,
}

fn blank(bytes: &mut [u8], at: usize, with: u8) {
    if let Some(byte) = bytes.get_mut(at) {
        *byte = with;
    }
}

fn holds(bytes: &[u8], at: usize, pattern: &[u8]) -> bool {
    bytes
        .get(at..)
        .is_some_and(|rest| rest.starts_with(pattern))
}

pub fn masked(text: &str) -> Masked {
    let bytes = text.as_bytes();
    let mut code = bytes.to_vec();
    let mut mask = bytes.to_vec();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'#' => {
                while bytes.get(at).is_some_and(|next| *next != b'\n') {
                    blank(&mut code, at, b' ');
                    blank(&mut mask, at, b' ');
                    at += 1;
                }
            }
            b'"' | b'\'' => {
                let closing = [byte; 3];
                let triple = holds(bytes, at, &closing);
                let width = if triple { 3 } else { 1 };
                at += width;
                while let Some(&inner) = bytes.get(at) {
                    if inner == b'\\' && bytes.get(at + 1).is_some() {
                        blank(&mut mask, at, b'_');
                        blank(&mut mask, at + 1, b'_');
                        at += 2;
                    } else if (triple && holds(bytes, at, &closing))
                        || (!triple && (inner == byte || inner == b'\n'))
                    {
                        at += width;
                        break;
                    } else {
                        if inner != b'\n' {
                            blank(&mut mask, at, b'_');
                        }
                        at += 1;
                    }
                }
            }
            _ => at += 1,
        }
    }
    Masked {
        code: String::from_utf8_lossy(&code).into_owned(),
        mask: String::from_utf8_lossy(&mask).into_owned(),
    }
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub struct CallArgs<'a> {
    pub code: &'a str,
    pub mask: &'a str,
}

pub fn calls<'a>(source: &'a Masked, name: &str) -> Vec<CallArgs<'a>> {
    let mask = source.mask.as_bytes();
    let needle = format!("{name}(");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = source.mask.get(from..).and_then(|rest| rest.find(&needle)) {
        let start = from + found;
        let open = start + needle.len();
        from = open;
        if !name.starts_with('.') {
            let before = source.mask.get(..start).unwrap_or_default();
            let prefix = before.trim_end_matches('_');
            if prefix
                .bytes()
                .last()
                .is_some_and(|byte| is_ident(byte) || byte == b'.')
                || prefix.trim_end_matches(' ').ends_with("def")
            {
                continue;
            }
        }
        let mut depth = 1usize;
        let mut close = open;
        while let Some(byte) = mask.get(close) {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            close += 1;
        }
        if let (Some(code), Some(mask)) =
            (source.code.get(open..close), source.mask.get(open..close))
        {
            out.push(CallArgs { code, mask });
        }
    }
    out
}

impl CallArgs<'_> {
    pub fn keyword(&self, key: &str) -> Option<&str> {
        let mut depth = 0usize;
        let mut starts = vec![0usize];
        for (at, byte) in self.mask.bytes().enumerate() {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => starts.push(at + 1),
                _ => {}
            }
        }
        starts.push(self.mask.len() + 1);
        starts.windows(2).find_map(|pair| {
            let &[start, next] = pair else {
                return None;
            };
            let end = next.saturating_sub(1).min(self.mask.len());
            let (name, _) = self.mask.get(start..end)?.split_once('=')?;
            let value_at = start + name.len() + 1;
            let value = self.mask.get(value_at..end)?;
            if name.trim() != key || value.starts_with('=') {
                return None;
            }
            self.code.get(value_at..end).map(str::trim)
        })
    }
}
