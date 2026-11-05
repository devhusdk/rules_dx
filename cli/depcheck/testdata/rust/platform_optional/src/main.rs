use anyhow::Result;

fn main() -> Result<()> {
    println!("{}", hello::greet("world")?);
    Ok(())
}
