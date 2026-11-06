use schemars::JsonSchema;
use schemars::generate::SchemaSettings;

use crate::exception::SecurityFile;
use crate::license_policy::LicensesFile;

fn render<T: JsonSchema>(title: &str) -> String {
    let generator = SchemaSettings::draft07().into_generator();
    let mut schema = generator.into_root_schema_for::<T>();
    schema.insert("title".to_owned(), title.into());
    format!("{:#}\n", schema.as_value())
}

/// Returns the JSON Schema for `security.toml`.
pub fn security_schema() -> String {
    render::<SecurityFile>("security.toml")
}

/// Returns the JSON Schema for `licenses.toml`.
pub fn licenses_schema() -> String {
    render::<LicensesFile>("licenses.toml")
}

#[cfg(test)]
#[path = "config_schema_tests.rs"]
mod config_schema_tests;
