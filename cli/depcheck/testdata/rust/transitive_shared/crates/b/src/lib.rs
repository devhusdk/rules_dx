use anyhow::Result;

pub fn label() -> String {
    String::from("shared-helper")
}

pub fn checked(name: &str) -> Result<String> {
    if name.is_empty() {
        anyhow::bail!("empty");
    }
    Ok(label())
}
