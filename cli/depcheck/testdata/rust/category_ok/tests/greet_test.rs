use anyhow::Result;
use test_helper::check;

#[test]
fn multi_category() -> Result<()> {
    assert!(check("world"));
    assert_eq!(hello::greet("world")?.contains("Hello"), true);
    Ok(())
}
