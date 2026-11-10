use serde_json::{Map, Value};

pub fn field<'a>(object: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))
}

pub fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    field(object, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at} '{key}' is not a string"))
}

pub fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, String> {
    field(object, key, at)?
        .as_u64()
        .ok_or_else(|| format!("{at} '{key}' is not a number"))
}

pub fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| format!("{at} '{key}' is not a boolean"))
}

pub fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, String> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| format!("{at} '{key}' is not a list"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at} '{key}' holds a non-string"))
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
            _ => Map::new(),
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
            field(&object(), "zz", "T").map(|_| ()),
            Err("T has no 'zz'".to_owned())
        );
    }

    #[test]
    fn reports_wrong_types() {
        let o = object();
        assert_eq!(
            string(&o, "n", "T"),
            Err("T 'n' is not a string".to_owned())
        );
        assert_eq!(
            number(&o, "s", "T"),
            Err("T 's' is not a number".to_owned())
        );
        assert_eq!(
            boolean(&o, "s", "T"),
            Err("T 's' is not a boolean".to_owned())
        );
        assert_eq!(strings(&o, "s", "T"), Err("T 's' is not a list".to_owned()));
        assert_eq!(
            strings(&o, "m", "T"),
            Err("T 'm' holds a non-string".to_owned())
        );
    }
}
