use schemars::generate::SchemaSettings;

use crate::config::DxConfig;

/// Returns the JSON Schema for `dx.toml`.
pub fn dx_schema() -> String {
    let generator = SchemaSettings::draft07().into_generator();
    let mut schema = generator.into_root_schema_for::<DxConfig>();
    schema.insert("title".to_owned(), "dx.toml".into());
    format!("{:#}\n", schema.as_value())
}
