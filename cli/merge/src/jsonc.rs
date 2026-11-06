//! Key and array edits of JSONC text that keep comments, trailing commas and untouched bytes.

use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::Value;

use crate::Error;

const BOM: &str = "\u{feff}";

/// The text with each value set at its key path; missing objects are created.
pub fn set_keys(text: &str, edits: &[(&[&str], Value)]) -> Result<String, Error> {
    edit(text, |root| {
        for (path, value) in edits {
            let (last, parents) = path.split_last().ok_or_else(empty_path)?;
            let object = walk(root.clone(), parents)?;
            match object.get(last) {
                Some(prop) => {
                    if prop.value().and_then(|node| node.to_serde_value()).as_ref() != Some(value) {
                        prop.set_value(input(value));
                    }
                }
                None => {
                    object.append(last, input(value));
                }
            }
        }
        Ok(())
    })
}

/// The text with `value` added to the array at `path` unless an equal entry is already there.
pub fn append_missing(text: &str, path: &[&str], value: Value) -> Result<String, Error> {
    edit(text, |root| {
        let (last, parents) = path.split_last().ok_or_else(empty_path)?;
        let object = walk(root.clone(), parents)?;
        let array = match object.get(last) {
            None => object.array_value_or_set(last),
            Some(prop) => prop
                .value()
                .and_then(|node| node.as_array())
                .ok_or_else(|| Error::WrongType(path.join(".")))?,
        };
        let present = array
            .elements()
            .iter()
            .any(|node| node.to_serde_value().as_ref() == Some(&value));
        if !present {
            array.append(input(&value));
        }
        Ok(())
    })
}

/// The text with the first key under `parents` that `matches` renamed to `new_name`, keeping its
/// value and position.
pub fn rename_key(
    text: &str,
    parents: &[&str],
    matches: impl Fn(&str) -> bool,
    new_name: &str,
) -> Result<String, Error> {
    edit(text, |root| {
        let object = match walk_existing(root.clone(), parents) {
            Some(object) => object,
            None => return Ok(()),
        };
        let found = object
            .properties()
            .into_iter()
            .enumerate()
            .find(|(_, prop)| {
                prop.decoded_name()
                    .is_some_and(|name| name != new_name && matches(&name))
            });
        if let Some((_, prop)) = found {
            if object.get(new_name).is_some() {
                return Ok(());
            }
            if let Some(name) = prop.name().and_then(|name| name.as_string_lit()) {
                name.set_raw_value(serde_json::to_string(new_name).unwrap_or_default());
            }
        }
        Ok(())
    })
}

fn walk_existing(mut object: CstObject, parents: &[&str]) -> Option<CstObject> {
    for name in parents {
        object = object.get(name)?.value()?.as_object()?;
    }
    Some(object)
}

/// The value of JSONC text, with comments and trailing commas dropped.
pub fn parse(text: &str) -> Result<Value, Error> {
    let body = text.strip_prefix(BOM).unwrap_or(text);
    let root = CstRootNode::parse(body, &ParseOptions::default())
        .map_err(|error| Error::Jsonc(error.to_string()))?;
    Ok(root
        .value()
        .and_then(|node| node.to_serde_value())
        .unwrap_or(Value::Null))
}

fn empty_path() -> Error {
    Error::WrongType(String::new())
}

fn edit(text: &str, apply: impl FnOnce(&CstObject) -> Result<(), Error>) -> Result<String, Error> {
    let (bom, body) = match text.strip_prefix(BOM) {
        Some(body) => (BOM, body),
        None => ("", text),
    };
    if body.trim().is_empty() {
        let root = CstRootNode::parse("{}", &ParseOptions::default())
            .map_err(|error| Error::Jsonc(error.to_string()))?;
        let object = root.object_value_or_set();
        apply(&object)?;
        let mut out = format!("{bom}{root}");
        out.push('\n');
        return Ok(out);
    }
    let root = CstRootNode::parse(body, &ParseOptions::default())
        .map_err(|error| Error::Jsonc(error.to_string()))?;
    let object = root
        .object_value()
        .ok_or_else(|| Error::WrongType("(root)".to_owned()))?;
    apply(&object)?;
    Ok(format!("{bom}{root}"))
}

fn walk(mut object: CstObject, parents: &[&str]) -> Result<CstObject, Error> {
    for name in parents {
        object = match object.get(name) {
            None => object.object_value_or_set(name),
            Some(prop) => prop
                .value()
                .and_then(|node| node.as_object())
                .ok_or_else(|| Error::WrongType((*name).to_owned()))?,
        };
    }
    Ok(object)
}

fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(flag) => CstInputValue::Bool(*flag),
        Value::Number(number) => CstInputValue::Number(number.to_string()),
        Value::String(text) => CstInputValue::String(text.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(input).collect()),
        Value::Object(map) => CstInputValue::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), input(item)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const USER: &str = "{\n  // mine\n  \"editor.tabSize\": 2, /* keep */\n  \"dx.a\": \"old\",\n  \"nest\": {\n    \"x\": 1,\n  },\n}\n";

    #[test]
    fn parse_ignores_comments_and_trailing_commas() {
        assert_eq!(parse(USER).unwrap()["nest"], json!({ "x": 1 }));
        assert_eq!(parse("").unwrap(), Value::Null);
    }

    #[test]
    fn rename_key_keeps_value_and_order() {
        let text = "{\n  \"f\": {\n    \"a:1\": {\"x\": 1}, // c\n    \"b\": {}\n  }\n}\n";
        let out = rename_key(text, &["f"], |name| name.starts_with("a:"), "a:2").unwrap();
        assert_eq!(out, text.replace("a:1", "a:2"));
        let none = rename_key(text, &["g"], |_| true, "z").unwrap();
        assert_eq!(none, text);
    }

    #[test]
    fn only_the_dx_key_changes() {
        let out = set_keys(USER, &[(&["dx.a"], json!("new"))]).unwrap();
        assert_eq!(out, USER.replace("\"old\"", "\"new\""));
    }

    #[test]
    fn a_missing_key_is_appended_and_user_bytes_stay() {
        let out = set_keys(USER, &[(&["nest", "y"], json!(true))]).unwrap();
        assert!(out.contains("// mine\n  \"editor.tabSize\": 2, /* keep */"));
        assert!(out.contains("\"y\": true"));
    }

    #[test]
    fn a_second_merge_writes_the_same_bytes() {
        let edits: [(&[&str], Value); 2] = [(&["dx.a"], json!("new")), (&["nest", "y"], json!(1))];
        let once = set_keys(USER, &edits).unwrap();
        assert_eq!(set_keys(&once, &edits).unwrap(), once);
    }

    #[test]
    fn equal_values_leave_the_text_alone() {
        assert_eq!(set_keys(USER, &[(&["dx.a"], json!("old"))]).unwrap(), USER);
    }

    #[test]
    fn empty_text_creates_the_file() {
        let out = set_keys("", &[(&["a", "b"], json!(1))]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap(),
            json!({"a": {"b": 1}})
        );
    }

    #[test]
    fn bom_and_crlf_survive() {
        let text = "\u{feff}{\r\n  \"a\": 1\r\n}\r\n";
        let out = set_keys(text, &[(&["a"], json!(2))]).unwrap();
        assert_eq!(out, "\u{feff}{\r\n  \"a\": 2\r\n}\r\n");
    }

    #[test]
    fn append_missing_adds_once() {
        let text = "{\n  // c\n  \"list\": [\"a\"]\n}\n";
        let once = append_missing(text, &["list"], json!("b")).unwrap();
        assert!(once.contains("// c") && once.contains("\"b\""));
        assert_eq!(append_missing(&once, &["list"], json!("b")).unwrap(), once);
        assert_eq!(append_missing(&once, &["list"], json!("a")).unwrap(), once);
    }

    #[test]
    fn append_missing_creates_the_array() {
        let out = append_missing("{}", &["hooks", "list"], json!({"c": 1})).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap(),
            json!({"hooks": {"list": [{"c": 1}]}})
        );
    }

    #[test]
    fn a_non_object_on_the_path_is_an_error() {
        assert!(set_keys("{\"a\": 1}", &[(&["a", "b"], json!(1))]).is_err());
        assert!(set_keys("{", &[(&["a"], json!(1))]).is_err());
    }
}
