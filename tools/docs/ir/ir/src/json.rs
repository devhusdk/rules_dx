use std::fmt;

use serde_json::{Map, Value};

/// What is wrong with one key of a JSON object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonProblem {
    Missing,
    NotString,
    NotNumber,
    NotBoolean,
    NotList,
    NonStringItem,
}

/// A missing or mistyped key of a JSON object, with the object's description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub at: String,
    pub key: String,
    pub problem: JsonProblem,
}

impl JsonError {
    fn new(at: &str, key: &str, problem: JsonProblem) -> Self {
        Self {
            at: at.to_owned(),
            key: key.to_owned(),
            problem,
        }
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { at, key, problem } = self;
        match problem {
            JsonProblem::Missing => write!(f, "{at} has no '{key}'"),
            JsonProblem::NotString => write!(f, "{at} '{key}' is not a string"),
            JsonProblem::NotNumber => write!(f, "{at} '{key}' is not a number"),
            JsonProblem::NotBoolean => write!(f, "{at} '{key}' is not a boolean"),
            JsonProblem::NotList => write!(f, "{at} '{key}' is not a list"),
            JsonProblem::NonStringItem => write!(f, "{at} '{key}' holds a non-string"),
        }
    }
}

impl std::error::Error for JsonError {}

/// Bridge for callers that still return `String`.
impl From<JsonError> for String {
    fn from(error: JsonError) -> Self {
        error.to_string()
    }
}

/// # Errors
///
/// Returns [`JsonProblem::Missing`] when the key is absent.
pub fn field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    at: &str,
) -> Result<&'a Value, JsonError> {
    object
        .get(key)
        .ok_or_else(|| JsonError::new(at, key, JsonProblem::Missing))
}

/// # Errors
///
/// Returns an error when the key is absent or not a string.
pub fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, JsonError> {
    field(object, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| JsonError::new(at, key, JsonProblem::NotString))
}

/// # Errors
///
/// Returns an error when the key is absent or not an unsigned number.
pub fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, JsonError> {
    field(object, key, at)?
        .as_u64()
        .ok_or_else(|| JsonError::new(at, key, JsonProblem::NotNumber))
}

/// # Errors
///
/// Returns an error when the key is absent or not a boolean.
pub fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, JsonError> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| JsonError::new(at, key, JsonProblem::NotBoolean))
}

/// # Errors
///
/// Returns an error when the key is absent or not a list.
pub fn array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    at: &str,
) -> Result<&'a Vec<Value>, JsonError> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| JsonError::new(at, key, JsonProblem::NotList))
}

/// # Errors
///
/// Returns an error when the key is absent, not a list, or holds a non-string.
pub fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, JsonError> {
    array(object, key, at)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| JsonError::new(at, key, JsonProblem::NonStringItem))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object() -> Map<String, Value> {
        match json!({"s": "a", "n": 3, "b": true, "l": ["x", "y"], "m": [1]}) {
            Value::Object(map) => map,
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_) => Map::new(),
        }
    }

    fn rejected<T: fmt::Debug>(result: Result<T, JsonError>) -> (JsonProblem, String) {
        let error = result.expect_err("rejected");
        (error.problem, error.to_string())
    }

    #[test]
    fn reads_each_type() {
        let o = object();
        assert_eq!(string(&o, "s", "T"), Ok("a".to_owned()));
        assert_eq!(number(&o, "n", "T"), Ok(3));
        assert_eq!(boolean(&o, "b", "T"), Ok(true));
        assert_eq!(
            strings(&o, "l", "T"),
            Ok(vec!["x".to_owned(), "y".to_owned()])
        );
        assert_eq!(array(&o, "m", "T"), Ok(&vec![json!(1)]));
    }

    #[test]
    fn reports_missing_key() {
        assert_eq!(
            rejected(field(&object(), "zz", "T")),
            (JsonProblem::Missing, "T has no 'zz'".to_owned())
        );
        assert_eq!(
            rejected(array(&object(), "zz", "T")),
            (JsonProblem::Missing, "T has no 'zz'".to_owned())
        );
    }

    #[test]
    fn reports_wrong_types() {
        let o = object();
        assert_eq!(
            rejected(string(&o, "n", "T")),
            (JsonProblem::NotString, "T 'n' is not a string".to_owned())
        );
        assert_eq!(
            rejected(number(&o, "s", "T")),
            (JsonProblem::NotNumber, "T 's' is not a number".to_owned())
        );
        assert_eq!(
            rejected(boolean(&o, "s", "T")),
            (JsonProblem::NotBoolean, "T 's' is not a boolean".to_owned())
        );
        assert_eq!(
            rejected(array(&o, "s", "T")),
            (JsonProblem::NotList, "T 's' is not a list".to_owned())
        );
        assert_eq!(
            rejected(strings(&o, "s", "T")),
            (JsonProblem::NotList, "T 's' is not a list".to_owned())
        );
        assert_eq!(
            rejected(strings(&o, "m", "T")),
            (
                JsonProblem::NonStringItem,
                "T 'm' holds a non-string".to_owned()
            )
        );
    }

    #[test]
    fn string_shim_keeps_the_text() {
        let text: String = field(&object(), "zz", "T")
            .map(|_| ())
            .map_err(String::from)
            .unwrap_err();
        assert_eq!(text, "T has no 'zz'");
    }
}
