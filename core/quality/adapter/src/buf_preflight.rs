//! Offline checks of a bound buf config before buf runs.

use saphyr_parser::{Event, Parser};

use crate::pointer_index::scan_yaml;

/// Refuses a `buf.yaml` (v1 or v2) that lists `deps`, and any alias, merge key or tag
/// that could hide them. buf would fetch those modules from the Buf Schema Registry.
///
/// # Errors
///
/// Returns a message naming the refused setting.
pub fn check_config(text: &str) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for item in Parser::new_from_str(text) {
        let (event, _) = item.map_err(|err| format!("buf config is not valid YAML: {err}"))?;
        let tagged = match &event {
            Event::Alias(_) => {
                return Err("buf config may not use YAML aliases".to_owned());
            }
            Event::Scalar(_, _, _, tag)
            | Event::MappingStart(_, tag)
            | Event::SequenceStart(_, tag) => tag.is_some(),
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart(_)
            | Event::DocumentEnd
            | Event::SequenceEnd
            | Event::MappingEnd => false,
        };
        if tagged {
            return Err("buf config may not use YAML tags".to_owned());
        }
    }
    let documents = scan_yaml(text, &[]).map_err(|err| {
        format!(
            "buf config is not valid YAML at {}:{}: {}",
            err.line + 1,
            err.col + 1,
            err.detail
        )
    })?;
    for doc in documents {
        for pointer in doc.pointers.keys() {
            let parts: Vec<&str> = pointer.split('/').skip(1).collect();
            if parts.contains(&"<<") {
                return Err("buf config may not use YAML merge keys".to_owned());
            }
            let deps = match parts.as_slice() {
                ["deps", _, ..] => Some("deps"),
                ["modules", _, "deps", _, ..] => Some("modules[].deps"),
                _ => None,
            };
            if let Some(key) = deps {
                return Err(format!(
                    "buf config may not list {key}: dx runs buf offline, so declare imported protos as proto_library deps instead"
                ));
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
    fn v1_deps_are_refused() {
        refused(
            "version: v1\ndeps:\n  - buf.build/googleapis/googleapis\n",
            "may not list deps",
        );
        refused(
            "version: v1\ndeps: [buf.build/acme/petapis]\n",
            "may not list deps",
        );
    }

    #[test]
    fn v2_deps_are_refused() {
        refused(
            "version: v2\ndeps:\n  - buf.build/googleapis/googleapis\n",
            "may not list deps",
        );
        refused(
            "version: v2\nmodules:\n  - path: proto\n  - path: vendor\n    deps:\n      - buf.build/acme/petapis\n",
            "modules[].deps",
        );
        refused(
            "version: v2\n---\ndeps: [buf.build/acme/petapis]\n",
            "may not list deps",
        );
    }

    #[test]
    fn configs_without_deps_pass() {
        check_config("version: v1\nlint:\n  use: [STANDARD]\n").expect("v1");
        check_config("version: v2\nmodules:\n  - path: proto\nlint:\n  use: [STANDARD]\n")
            .expect("v2");
        check_config("version: v2\ndeps: []\nmodules:\n  - path: proto\n    deps: []\n")
            .expect("empty deps");
        check_config("version: v2\ndeps:\n").expect("null deps");
        check_config("\u{feff}version: v2\n").expect("bom");
        check_config("").expect("empty");
    }

    #[test]
    fn aliases_merge_keys_tags_and_bad_yaml_are_refused() {
        refused(
            "base: &d [buf.build/acme/petapis]\nversion: v2\ndeps: *d\n",
            "aliases",
        );
        refused(
            "version: v2\nmodules:\n  - <<: {deps: [buf.build/acme/petapis]}\n    path: proto\n",
            "merge keys",
        );
        refused("version: !!str v2\n", "tags");
        refused("deps: [\n", "not valid YAML");
    }
}
