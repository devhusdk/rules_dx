//! Reads the data of a rockspec without running Lua.

use std::collections::BTreeMap;

/// A value of the Lua subset rockspecs are written in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// A string or a number.
    Text(String),
    /// `true` or `false`.
    Flag(bool),
    /// `nil`.
    Nil,
    /// A table constructor.
    Table(Table),
}

/// A table constructor: positional items and named fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Table {
    /// The positional items.
    pub list: Vec<Value>,
    /// The named fields.
    pub map: BTreeMap<String, Value>,
}

impl Value {
    /// The text of a string or number.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The table of a table constructor.
    #[must_use]
    pub fn table(&self) -> Option<&Table> {
        match self {
            Self::Table(table) => Some(table),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Name(String),
    Text(String),
    Number(String),
    Punct(&'static str),
}

const PUNCT: [&str; 10] = ["..", "{", "}", "[", "]", "(", ")", "=", ",", ";"];

fn long_bracket(chars: &[char], at: usize) -> Option<(usize, usize)> {
    if chars.get(at) != Some(&'[') {
        return None;
    }
    let mut level = 0;
    while chars.get(at + 1 + level) == Some(&'=') {
        level += 1;
    }
    (chars.get(at + 1 + level) == Some(&'[')).then_some((level, at + 2 + level))
}

fn long_end(chars: &[char], from: usize, level: usize) -> Option<usize> {
    let close: Vec<char> = std::iter::once(']')
        .chain(std::iter::repeat_n('=', level))
        .chain(std::iter::once(']'))
        .collect();
    (from..chars.len()).find(|&at| chars[at..].starts_with(&close))
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
        } else if c == '-' && chars.get(at + 1) == Some(&'-') {
            at += 2;
            if let Some((level, body)) = long_bracket(&chars, at) {
                let end = long_end(&chars, body, level).ok_or("unterminated comment")?;
                at = end + level + 2;
            } else {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
        } else if c == '"' || c == '\'' {
            let mut value = String::new();
            at += 1;
            loop {
                match chars.get(at) {
                    None | Some('\n') => return Err("unterminated string".to_owned()),
                    Some('\\') => {
                        let next = chars.get(at + 1).ok_or("unterminated string")?;
                        value.push(match next {
                            'n' => '\n',
                            't' => '\t',
                            other => *other,
                        });
                        at += 2;
                    }
                    Some(&end) if end == c => {
                        at += 1;
                        break;
                    }
                    Some(&other) => {
                        value.push(other);
                        at += 1;
                    }
                }
            }
            tokens.push(Token::Text(value));
        } else if let Some((level, body)) = long_bracket(&chars, at) {
            let end = long_end(&chars, body, level).ok_or("unterminated string")?;
            let value: String = chars[body..end].iter().collect();
            tokens.push(Token::Text(
                value.strip_prefix('\n').unwrap_or(&value).to_owned(),
            ));
            at = end + level + 2;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            tokens.push(Token::Name(chars[start..at].iter().collect()));
        } else if c.is_ascii_digit() {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '.') {
                at += 1;
            }
            tokens.push(Token::Number(chars[start..at].iter().collect()));
        } else if let Some(punct) = PUNCT
            .iter()
            .find(|punct| chars[at..].starts_with(&punct.chars().collect::<Vec<_>>()))
        {
            tokens.push(Token::Punct(punct));
            at += punct.len();
        } else {
            return Err(format!("unsupported character {c:?}"));
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    at: usize,
    vars: &'a mut BTreeMap<String, Value>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn eat(&mut self, punct: &str) -> bool {
        if matches!(self.peek(), Some(Token::Punct(found)) if *found == punct) {
            self.at += 1;
            return true;
        }
        false
    }

    fn expect(&mut self, punct: &str) -> Result<(), String> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(format!("expected {punct:?}, found {:?}", self.peek()))
        }
    }

    fn expression(&mut self) -> Result<Value, String> {
        let mut value = self.primary()?;
        while self.eat("..") {
            let right = self.primary()?;
            match (value.text(), right.text()) {
                (Some(left), Some(right)) => value = Value::Text(format!("{left}{right}")),
                _ => return Err("only strings are concatenated".to_owned()),
            }
        }
        Ok(value)
    }

    fn primary(&mut self) -> Result<Value, String> {
        let token = self
            .peek()
            .cloned()
            .ok_or("the file ends inside an expression")?;
        match token {
            Token::Text(text) | Token::Number(text) => {
                self.at += 1;
                Ok(Value::Text(text))
            }
            Token::Name(name) => {
                self.at += 1;
                match name.as_str() {
                    "true" => Ok(Value::Flag(true)),
                    "false" => Ok(Value::Flag(false)),
                    "nil" => Ok(Value::Nil),
                    _ => self.vars.get(&name).cloned().ok_or_else(|| {
                        format!(
                            "{name} is not a plain value; rockspecs may only use strings and tables"
                        )
                    }),
                }
            }
            Token::Punct("{") => {
                self.at += 1;
                self.table()
            }
            other => Err(format!("unsupported expression at {other:?}")),
        }
    }

    fn table(&mut self) -> Result<Value, String> {
        let mut table = Table::default();
        while !self.eat("}") {
            match (self.peek().cloned(), self.tokens.get(self.at + 1)) {
                (Some(Token::Name(key)), Some(Token::Punct("="))) => {
                    self.at += 2;
                    table.map.insert(key, self.expression()?);
                }
                (Some(Token::Punct("[")), _) => {
                    self.at += 1;
                    let key = self.expression()?;
                    self.expect("]")?;
                    self.expect("=")?;
                    let key = key.text().ok_or("a table key must be a string")?.to_owned();
                    table.map.insert(key, self.expression()?);
                }
                _ => table.list.push(self.expression()?),
            }
            if !self.eat(",") && !self.eat(";") {
                self.expect("}")?;
                break;
            }
        }
        Ok(Value::Table(table))
    }

    fn chunk(&mut self) -> Result<BTreeMap<String, Value>, String> {
        let mut globals = BTreeMap::new();
        while let Some(token) = self.peek().cloned() {
            let local = token == Token::Name("local".to_owned());
            if local {
                self.at += 1;
            }
            let Some(Token::Name(name)) = self.peek().cloned() else {
                return Err(format!("unsupported statement at {token:?}"));
            };
            self.at += 1;
            self.expect("=")?;
            let value = self.expression()?;
            self.vars.insert(name.clone(), value.clone());
            if !local {
                globals.insert(name, value);
            }
            self.eat(";");
        }
        Ok(globals)
    }
}

/// The top-level assignments of a rockspec.
///
/// # Errors
/// Fails when the file uses more than assignments of strings and tables.
pub fn parse(text: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut vars = BTreeMap::new();
    let tokens = tokenize(text)?;
    Parser {
        tokens,
        at: 0,
        vars: &mut vars,
    }
    .chunk()
}
