use anyhow::{Context, Result};

pub fn greet(name: &str) -> Result<String> {
    if name.is_empty() {
        anyhow::bail!("name must not be empty");
    }
    Ok(format!("Hello, {name}!"))
}

pub fn add(left: u64, right: u64) -> u64 {
    left.wrapping_add(right)
}

pub fn parse(s: &str) -> Result<u64> {
    s.parse::<u64>().context("parse")
}
