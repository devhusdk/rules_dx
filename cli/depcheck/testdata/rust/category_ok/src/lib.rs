use anyhow::Result;
use test_helper::check;

pub fn greet(name: &str) -> Result<String> {
    assert!(check(name));
    if name.is_empty() {
        anyhow::bail!("empty");
    }
    Ok(format!("Hello, {name}!"))
}
