use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

use crate::config::DxConfig;
use crate::curated_defaults_gen::{FAMILIES, OPT_IN_FAMILIES, TOOLS};

/// Returns the JSON Schema for `dx.toml`.
pub fn dx_schema() -> String {
    let generator = SchemaSettings::draft07().into_generator();
    let mut schema = generator.into_root_schema_for::<DxConfig>();
    schema.insert("title".to_owned(), "dx.toml".into());
    let mut value = schema.to_value();
    add_family_ids(&mut value);
    add_tool_ids(&mut value);
    format!("{:#}\n", value)
}

fn add_family_ids(schema: &mut Value) {
    let mut ids: Vec<&str> = FAMILIES
        .iter()
        .map(|family| family.id)
        .chain(OPT_IN_FAMILIES.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    schema["properties"]["families"]["propertyNames"] = json!({ "enum": ids });
}

/// A tool id, or a `+id` or `-id` edit of the curated list.
fn tool_id_items() -> Value {
    let edits: Vec<String> = TOOLS
        .iter()
        .flat_map(|id| [format!("+{id}"), format!("-{id}")])
        .collect();
    json!({ "anyOf": [{ "enum": TOOLS }, { "enum": edits }] })
}

fn add_tool_ids(schema: &mut Value) {
    let items = tool_id_items();
    for capability in ["lint", "format", "typecheck", "audit"] {
        schema["definitions"]["FamilyConfig"]["properties"][capability]["items"] = items.clone();
    }
    schema["properties"]["skip_tools"]["items"] = json!({ "enum": TOOLS });
}
