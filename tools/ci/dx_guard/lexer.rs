//! A lexer for Rust source that keeps words, string literals and punctuation.

/// One token of Rust source. Comments and character literals are dropped.
#[derive(Debug, PartialEq)]
pub enum Token {
    Word(String),
    Text(String),
    Punct(char),
}

/// Lexes Rust source into words, string literals and punctuation.
pub fn rust_tokens(source: &str) -> Vec<Token> {
    Lexer {
        chars: source.chars().collect(),
        at: 0,
        out: Vec::new(),
    }
    .run()
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    out: Vec<Token>,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn run(mut self) -> Vec<Token> {
        while let Some(current) = self.peek(0) {
            if current.is_whitespace() {
                self.at += 1;
            } else if current == '/' && self.peek(1) == Some('/') {
                while self.peek(0).is_some_and(|next| next != '\n') {
                    self.at += 1;
                }
            } else if current == '/' && self.peek(1) == Some('*') {
                self.block_comment();
            } else if current == '"' {
                self.at += 1;
                self.quoted();
            } else if current == '\'' {
                self.quote();
            } else if current.is_alphanumeric() || current == '_' {
                self.word();
            } else {
                self.out.push(Token::Punct(current));
                self.at += 1;
            }
        }
        self.out
    }

    fn block_comment(&mut self) {
        let mut depth = 0usize;
        while let Some(current) = self.peek(0) {
            if current == '/' && self.peek(1) == Some('*') {
                depth += 1;
                self.at += 2;
            } else if current == '*' && self.peek(1) == Some('/') {
                depth -= 1;
                self.at += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.at += 1;
            }
        }
    }

    /// Reads a string body after its opening quote.
    fn quoted(&mut self) {
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            self.at += 1;
            match current {
                '"' => break,
                '\\' => {
                    let escaped = self.peek(0).unwrap_or('\\');
                    self.at += 1;
                    match escaped {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        '\n' => {
                            while self.peek(0).is_some_and(char::is_whitespace) {
                                self.at += 1;
                            }
                        }
                        other => text.push(other),
                    }
                }
                other => text.push(other),
            }
        }
        self.out.push(Token::Text(text));
    }

    /// Reads a raw string body after its opening quote, closed by a quote and `hashes` hashes.
    fn raw(&mut self, hashes: usize) {
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            self.at += 1;
            if current == '"' && (0..hashes).all(|index| self.peek(index) == Some('#')) {
                self.at += hashes;
                break;
            }
            text.push(current);
        }
        self.out.push(Token::Text(text));
    }

    /// Skips a character literal, or the quote of a lifetime.
    fn quote(&mut self) {
        if self.peek(1) == Some('\\') {
            self.at += 3;
            while self.peek(0).is_some_and(|next| next != '\'') {
                self.at += 1;
            }
            self.at += 1;
        } else if self.peek(2) == Some('\'') {
            self.at += 3;
        } else {
            self.at += 1;
        }
    }

    fn word(&mut self) {
        let start = self.at;
        while self
            .peek(0)
            .is_some_and(|next| next.is_alphanumeric() || next == '_')
        {
            self.at += 1;
        }
        let word: String = self
            .chars
            .get(start..self.at)
            .unwrap_or_default()
            .iter()
            .collect();
        let prefix = matches!(word.as_str(), "b" | "c" | "r" | "br" | "cr");
        let mut hashes = 0;
        while word.ends_with('r') && self.peek(hashes) == Some('#') {
            hashes += 1;
        }
        if prefix && self.peek(hashes) == Some('"') {
            self.at += hashes + 1;
            if word.ends_with('r') {
                self.raw(hashes);
            } else {
                self.quoted();
            }
        } else if word == "b" && self.peek(0) == Some('\'') {
            self.quote();
        } else {
            self.out.push(Token::Word(word));
        }
    }
}
