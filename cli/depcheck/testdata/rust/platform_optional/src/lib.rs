use anyhow::Result;

pub fn greet(name: &str) -> Result<String> {
    if name.is_empty() {
        anyhow::bail!("empty");
    }
    Ok(format!("Hello, {name}!"))
}

#[cfg(target_os = "windows")]
pub fn windows_only() -> String {
    win_only::label()
}

#[cfg(feature = "myfeat")]
pub fn feat_only() -> String {
    optional_feat::label()
}
