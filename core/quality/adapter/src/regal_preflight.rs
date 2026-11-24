use saphyr_parser::{Event, Parser};

use crate::commands::plain_relative;
use crate::pointer_index::scan_yaml;

const URL: &str = "/capabilities/from/url";
const FILE: &str = "/capabilities/from/file";

pub fn check_config(text: &str) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for item in Parser::new_from_str(text) {
        let (event, _) = item.map_err(|err| format!("regal config is not valid YAML: {err}"))?;
        let tagged = match &event {
            Event::Alias(_) => {
                return Err("regal config may not use YAML aliases".to_owned());
            }
            Event::Scalar(_, _, _, tag)
            | Event::MappingStart(_, tag)
            | Event::SequenceStart(_, tag) => tag.is_some(),
            _ => false,
        };
        if tagged {
            return Err("regal config may not use YAML tags".to_owned());
        }
    }
    let documents = scan_yaml(text, &[FILE]).map_err(|err| {
        format!(
            "regal config is not valid YAML at {}:{}: {}",
            err.line + 1,
            err.col + 1,
            err.detail
        )
    })?;
    for doc in documents {
        if doc
            .pointers
            .keys()
            .any(|pointer| pointer.split('/').any(|part| part == "<<"))
        {
            return Err("regal config may not use YAML merge keys".to_owned());
        }
        if doc.pointers.contains_key(URL) {
            return Err(
                "regal config may not set capabilities.from.url: dx runs regal offline".to_owned(),
            );
        }
        if doc.pointers.contains_key(FILE) {
            match doc.captured.get(FILE) {
                Some(rel) if plain_relative(rel) => {}
                Some(rel) => {
                    return Err(format!(
                        "regal config capabilities.from.file must be a relative forward-slash path without `..`, got {rel:?}"
                    ));
                }
                None => {
                    return Err("regal config capabilities.from.file must be a string".to_owned());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_config;

    fn refused(text: &str, needle: &str) {
        let err = check_config(text).expect_err(text);
        assert!(err.contains(needle), "{text:?}: {err}");
    }

    #[test]
    fn url_capabilities_are_refused() {
        refused(
            "capabilities:\n  from:\n    url: https://example.com/caps.json\n",
            "capabilities.from.url",
        );
        refused(
            "capabilities:\n  from:\n    url: file:///etc/caps.json\n",
            "capabilities.from.url",
        );
        refused(
            "capabilities: {from: {url: 'https://example.com/c.json'}}\n",
            "capabilities.from.url",
        );
        refused(
            "rules: {}\n---\ncapabilities:\n  from:\n    url: https://example.com/c.json\n",
            "capabilities.from.url",
        );
    }

    #[test]
    fn file_capabilities_must_stay_relative() {
        for rel in [
            "/etc/caps.json",
            "C:/caps.json",
            "../c.json",
            "a/../../b",
            "a\\b",
            "./c.json",
            "",
        ] {
            refused(
                &format!("capabilities:\n  from:\n    file: {rel:?}\n"),
                "capabilities.from.file",
            );
        }
        refused(
            "capabilities:\n  from:\n    file: [a.json]\n",
            "must be a string",
        );
        check_config("capabilities:\n  from:\n    file: policy/caps.json\n")
            .expect("relative file");
    }

    #[test]
    fn engine_versions_and_plain_configs_pass() {
        check_config("capabilities:\n  from:\n    engine: opa\n    version: v1.4.0\n")
            .expect("engine");
        check_config("rules:\n  style:\n    line-length:\n      level: error\n").expect("rules");
        check_config("\u{feff}rules: {}\n").expect("bom");
        check_config("").expect("empty");
    }

    #[test]
    fn aliases_merge_keys_tags_and_bad_yaml_are_refused() {
        refused(
            "base: &caps\n  url: https://example.com/c.json\ncapabilities:\n  from: *caps\n",
            "aliases",
        );
        refused(
            "capabilities:\n  from:\n    <<: {url: https://example.com/c.json}\n",
            "merge keys",
        );
        refused(
            "capabilities:\n  from:\n    file: !!binary L2V0Yy9jYXBzLmpzb24=\n",
            "tags",
        );
        refused("rules: [\n", "not valid YAML");
    }
}
