//! Shared evaluation core for native, browser, and mobile outputs.
//!
//! The parser is intentionally dependency-free so the same source compiles
//! for the host, `wasm32-unknown-unknown`, and Android targets. fallible
//! inputs report [`EvalError`] instead of trapping, which lets every caller
//! surface the failure without a process abort.

pub const CORE_VERSION: &str = "1.0.0";
pub const MAX_INPUT_LEN: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum EvalError {
    Empty,
    Unexpected(char, usize),
    UnbalancedParen,
    DivisionByZero,
    Overflow,
    InputTooLong,
}

impl EvalError {
    pub fn kind(&self) -> &'static str {
        match self {
            EvalError::Empty => "empty",
            EvalError::Unexpected(_, _) => "unexpected",
            EvalError::UnbalancedParen => "unbalanced",
            EvalError::DivisionByZero => "division_by_zero",
            EvalError::Overflow => "overflow",
            EvalError::InputTooLong => "input_too_long",
        }
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Empty => write!(f, "expression is empty"),
            EvalError::Unexpected(found, at) => {
                write!(f, "unexpected {found:?} at byte {at}")
            }
            EvalError::UnbalancedParen => write!(f, "unbalanced parenthesis"),
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::Overflow => write!(f, "integer overflow"),
            EvalError::InputTooLong => {
                write!(f, "input exceeds {MAX_INPUT_LEN} bytes")
            }
        }
    }
}

impl std::error::Error for EvalError {}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            bytes: input.as_bytes(),
            at: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<i64, EvalError> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_spaces();
            let op = match self.peek() {
                Some(b'+') => '+',
                Some(b'-') => '-',
                _ => return Ok(value),
            };
            self.at += 1;
            let rhs = self.parse_term()?;
            value = match op {
                '+' => value.checked_add(rhs),
                _ => value.checked_sub(rhs),
            }
            .ok_or(EvalError::Overflow)?;
        }
    }

    fn parse_term(&mut self) -> Result<i64, EvalError> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_spaces();
            let op = match self.peek() {
                Some(b'*') => '*',
                Some(b'/') => '/',
                Some(b'%') => '%',
                _ => return Ok(value),
            };
            self.at += 1;
            let rhs = self.parse_factor()?;
            value = match op {
                '*' => value.checked_mul(rhs).ok_or(EvalError::Overflow)?,
                '/' => {
                    if rhs == 0 {
                        return Err(EvalError::DivisionByZero);
                    }
                    value.checked_div(rhs).ok_or(EvalError::Overflow)?
                }
                _ => {
                    if rhs == 0 {
                        return Err(EvalError::DivisionByZero);
                    }
                    value.checked_rem(rhs).ok_or(EvalError::Overflow)?
                }
            };
        }
    }

    fn parse_factor(&mut self) -> Result<i64, EvalError> {
        self.skip_spaces();
        match self.peek() {
            Some(b'(') => {
                self.at += 1;
                let value = self.parse_expr()?;
                self.skip_spaces();
                match self.peek() {
                    Some(b')') => {
                        self.at += 1;
                        Ok(value)
                    }
                    _ => Err(EvalError::UnbalancedParen),
                }
            }
            Some(b'-') => {
                self.at += 1;
                self.parse_factor()?
                    .checked_neg()
                    .ok_or(EvalError::Overflow)
            }
            Some(b'+') => {
                self.at += 1;
                self.parse_factor()
            }
            Some(b) if b.is_ascii_digit() => self.parse_number(),
            Some(_) => {
                let at = self.at;
                let found = self.bytes[at] as char;
                Err(EvalError::Unexpected(found, at))
            }
            None => Err(EvalError::UnbalancedParen),
        }
    }

    fn parse_number(&mut self) -> Result<i64, EvalError> {
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        self.bytes[start..self.at]
            .iter()
            .try_fold(0i64, |value, digit| {
                value
                    .checked_mul(10)
                    .and_then(|scaled| scaled.checked_add(i64::from(digit - b'0')))
                    .ok_or(EvalError::Overflow)
            })
    }
}

pub fn eval_expr(input: &str) -> Result<i64, EvalError> {
    if input.len() > MAX_INPUT_LEN {
        return Err(EvalError::InputTooLong);
    }
    let mut parser = Parser::new(input);
    parser.skip_spaces();
    if parser.peek().is_none() {
        return Err(EvalError::Empty);
    }
    let value = parser.parse_expr()?;
    parser.skip_spaces();
    match parser.peek() {
        None => Ok(value),
        Some(_) => {
            let at = parser.at;
            Err(EvalError::Unexpected(parser.bytes[at] as char, at))
        }
    }
}

#[no_mangle]
pub extern "C" fn shared_probe() -> i64 {
    eval_expr("6 * 7").unwrap_or(-1)
}

#[cfg(target_arch = "wasm32")]
pub mod bind {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn evaluate(input: &str) -> String {
        match super::eval_expr(input) {
            Ok(value) => format!("ok:{value}"),
            Err(error) => format!("err:{}", error.kind()),
        }
    }

    #[wasm_bindgen]
    pub fn core_version() -> String {
        super::CORE_VERSION.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_parens() {
        assert_eq!(eval_expr("2 + 3 * 4"), Ok(14));
        assert_eq!(eval_expr("2 * (3 + 4)"), Ok(14));
        assert_eq!(eval_expr("(2 + 3) * 4"), Ok(20));
    }

    #[test]
    fn unary_and_remainder() {
        assert_eq!(eval_expr("-3 + 10 % 4"), Ok(-1));
        assert_eq!(eval_expr("--5"), Ok(5));
    }

    #[test]
    fn invalid_inputs_report_kinds() {
        assert_eq!(eval_expr(""), Err(EvalError::Empty));
        assert_eq!(eval_expr("   "), Err(EvalError::Empty));
        assert_eq!(eval_expr("2 + x"), Err(EvalError::Unexpected('x', 4)));
        assert_eq!(eval_expr("(2 + 3"), Err(EvalError::UnbalancedParen));
        assert_eq!(eval_expr("2 + 3)"), Err(EvalError::Unexpected(')', 5)));
        assert_eq!(eval_expr("1 / 0"), Err(EvalError::DivisionByZero));
        assert_eq!(eval_expr("1 % 0"), Err(EvalError::DivisionByZero));
        assert_eq!(
            eval_expr("9223372036854775807 + 1"),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            eval_expr("-9223372036854775808 - 1"),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            eval_expr(&"1".repeat(MAX_INPUT_LEN + 1)),
            Err(EvalError::InputTooLong)
        );
    }

    #[test]
    fn probe_matches_direct_eval() {
        assert_eq!(
            shared_probe(),
            eval_expr("6 * 7").expect("fixed probe input")
        );
    }

    #[test]
    fn error_kinds_are_stable() {
        assert_eq!(EvalError::Empty.kind(), "empty");
        assert_eq!(EvalError::Unexpected('?', 0).kind(), "unexpected");
        assert_eq!(EvalError::UnbalancedParen.kind(), "unbalanced");
        assert_eq!(EvalError::DivisionByZero.kind(), "division_by_zero");
        assert_eq!(EvalError::Overflow.kind(), "overflow");
        assert_eq!(EvalError::InputTooLong.kind(), "input_too_long");
    }
}
