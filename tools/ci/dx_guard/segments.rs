#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    Code,
    Comment,
    Text,
    Doc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub piece: Piece,
    pub text: String,
}

struct Splitter<'a> {
    source: &'a str,
    bytes: &'a [u8],
    out: Vec<Segment>,
    code_start: usize,
}

impl<'a> Splitter<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            out: Vec::new(),
            code_start: 0,
        }
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn at(&self, index: usize) -> Option<u8> {
        self.bytes.get(index).copied()
    }

    fn starts(&self, index: usize, prefix: &[u8]) -> bool {
        self.bytes
            .get(index..)
            .is_some_and(|rest| rest.starts_with(prefix))
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        self.source.get(start..end).unwrap_or_default()
    }

    fn line_end(&self, from: usize) -> usize {
        self.bytes
            .get(from..)
            .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
            .map_or(self.len(), |offset| from + offset)
    }

    fn push(&mut self, piece: Piece, text: &str) {
        if !text.is_empty() || piece != Piece::Code {
            self.out.push(Segment {
                piece,
                text: text.to_owned(),
            });
        }
    }

    fn piece(&mut self, at: usize, piece: Piece, text: &str, resume: usize) {
        let code = self.slice(self.code_start, at);
        self.push(Piece::Code, code);
        self.push(piece, text);
        self.code_start = resume;
    }

    fn finish(mut self) -> Vec<Segment> {
        let code = self.slice(self.code_start, self.len());
        self.push(Piece::Code, code);
        self.out
    }
}

pub fn starlark_segments(source: &str) -> Vec<Segment> {
    let mut split = Splitter::new(source);
    let (mut index, mut line_start) = (0, 0);
    while let Some(byte) = split.at(index) {
        match byte {
            b'\n' => {
                index += 1;
                line_start = index;
            }
            b'#' => {
                let end = split.line_end(index);
                split.piece(index, Piece::Comment, split.slice(index + 1, end), end);
                index = end;
            }
            quote @ (b'"' | b'\'') => {
                let closer = [quote; 3];
                let width = if split.starts(index, &closer) { 3 } else { 1 };
                let closer = closer.get(..width).unwrap_or_default();
                let before = split.slice(line_start, index).trim();
                let doc = (width == 3 && before.is_empty())
                    || before.ends_with("doc =")
                    || before.ends_with("doc=");
                let start = index + width;
                let mut end = start;
                while let Some(next) = split.at(end) {
                    if next == b'\\' {
                        end += 2;
                    } else if split.starts(end, closer) {
                        break;
                    } else {
                        end += 1;
                    }
                }
                let end = end.min(split.len());
                let resume = (end + width).min(split.len());
                let piece = if doc { Piece::Doc } else { Piece::Text };
                split.piece(index, piece, split.slice(start, end), resume);
                index = resume;
                if let Some(newline) = split.slice(start.min(index), index).rfind('\n') {
                    line_start = start + newline + 1;
                }
            }
            _ => index += 1,
        }
    }
    split.finish()
}

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn raw_string(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let joined = index
        .checked_sub(1)
        .and_then(|before| bytes.get(before))
        .is_some_and(|byte| ident_byte(*byte));
    if bytes.get(index) != Some(&b'r') || joined {
        return None;
    }
    let hashes = bytes
        .get(index + 1..)
        .unwrap_or_default()
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    let open = index + 1 + hashes;
    if bytes.get(open) != Some(&b'"') {
        return None;
    }
    let mut end = open + 1;
    while let Some(byte) = bytes.get(end) {
        let closed = bytes
            .get(end + 1..)
            .unwrap_or_default()
            .iter()
            .take(hashes)
            .filter(|byte| **byte == b'#')
            .count()
            == hashes;
        if *byte == b'"' && closed {
            return Some((open + 1, end));
        }
        end += 1;
    }
    Some((open + 1, bytes.len()))
}

fn char_literal(source: &str, index: usize) -> Option<usize> {
    let rest = source.get(index + 1..)?;
    if let Some(escaped) = rest.strip_prefix('\\') {
        return escaped.get(1..)?.find('\'').map(|offset| offset + 4);
    }
    let first = rest.chars().next()?;
    rest.get(first.len_utf8()..)?
        .starts_with('\'')
        .then_some(first.len_utf8() + 2)
}

pub fn rust_segments(source: &str) -> Vec<Segment> {
    let mut split = Splitter::new(source);
    let mut index = 0;
    while let Some(byte) = split.at(index) {
        if split.starts(index, b"//") {
            let end = split.line_end(index);
            split.piece(index, Piece::Comment, split.slice(index + 2, end), end);
            index = end;
        } else if split.starts(index, b"/*") {
            let (mut end, mut depth) = (index + 2, 1_usize);
            while end < split.len() && depth > 0 {
                if split.starts(end, b"/*") {
                    depth += 1;
                    end += 2;
                } else if split.starts(end, b"*/") {
                    depth -= 1;
                    end += 2;
                } else {
                    end += 1;
                }
            }
            let end = end.min(split.len());
            let text = split.slice(index + 2, end).trim_end_matches("*/");
            split.piece(index, Piece::Comment, text, end);
            index = end;
        } else if let Some((start, end)) = raw_string(split.bytes, index) {
            let hashes = start - index - 2;
            let resume = (end + 1 + hashes).min(split.len());
            split.piece(index, Piece::Text, split.slice(start, end), resume);
            index = resume;
        } else if byte == b'"' {
            let mut end = index + 1;
            while let Some(next) = split.at(end) {
                if next == b'"' {
                    break;
                }
                end += if next == b'\\' { 2 } else { 1 };
            }
            let end = end.min(split.len());
            let resume = (end + 1).min(split.len());
            split.piece(index, Piece::Text, split.slice(index + 1, end), resume);
            index = resume;
        } else if byte == b'\'' {
            index += char_literal(source, index).unwrap_or(1);
        } else {
            index += 1;
        }
    }
    split.finish()
}

pub fn count_word(text: &str, word: &str) -> usize {
    let bytes = text.as_bytes();
    let joined = |at: Option<usize>| {
        at.and_then(|at| bytes.get(at))
            .is_some_and(|byte| ident_byte(*byte))
    };
    text.match_indices(word)
        .filter(|(start, _)| !joined(start.checked_sub(1)) && !joined(Some(start + word.len())))
        .count()
}
