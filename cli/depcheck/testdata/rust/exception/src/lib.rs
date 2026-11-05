use anyhow::Result;

pub fn greet(name: &str) -> Result<String> {
    if name.is_empty() {
        anyhow::bail!("empty");
    }
    Ok(format!("Hello, {name}!"))
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
