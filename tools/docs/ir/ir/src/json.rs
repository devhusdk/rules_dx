use serde_json::{Map, Value};

/// A required JSON field that is missing or has the wrong type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    #[error("{at} has no '{key}'")]
    Missing { at: String, key: String },
    #[error("{at} '{key}' is not a {expected}")]
    WrongType {
        at: String,
        key: String,
        expected: &'static str,
    },
    #[error("{at} '{key}' holds a non-string")]
    NonString { at: String, key: String },
}

fn wrong(at: &str, key: &str, expected: &'static str) -> FieldError {
    FieldError::WrongType {
        at: at.to_owned(),
        key: key.to_owned(),
        expected,
    }
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn field<'a>(object: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a Value, FieldError> {
    object
        .get(key)
        .ok_or_else(|| FieldError::Missing {
            at: at.to_owned(),
            key: key.to_owned(),
        })
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, FieldError> {
    field(object, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| wrong(at, key, "string"))
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, FieldError> {
    field(object, key, at)?
        .as_u64()
        .ok_or_else(|| wrong(at, key, "number"))
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, FieldError> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| wrong(at, key, "boolean"))
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, FieldError> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| wrong(at, key, "list"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| FieldError::NonString {
                    at: at.to_owned(),
                    key: key.to_owned(),
                })
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
    }

    #[test]
    fn reports_missing_key() {
        assert_eq!(
            field(&object(), "zz", "T").map(|_| ()).map_err(|e| e.to_string()),
            Err("T has no 'zz'".to_owned())
        );
    }

    #[test]
    fn reports_wrong_types() {
        let o = object();
        assert_eq!(
            string(&o, "n", "T").map_err(|e| e.to_string()),
            Err("T 'n' is not a string".to_owned())
        );
        assert_eq!(
            number(&o, "s", "T").map_err(|e| e.to_string()),
            Err("T 's' is not a number".to_owned())
        );
        assert_eq!(
            boolean(&o, "s", "T").map_err(|e| e.to_string()),
            Err("T 's' is not a boolean".to_owned())
        );
        assert_eq!(strings(&o, "s", "T").map_err(|e| e.to_string()), Err("T 's' is not a list".to_owned()));
        assert_eq!(
            strings(&o, "m", "T").map_err(|e| e.to_string()),
            Err("T 'm' holds a non-string".to_owned())
        );
    }
}
