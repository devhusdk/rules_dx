use anyhow::Result;
use test_helper::check;

#[test]
fn uses_prod_only_in_test() -> Result<()> {
    assert!(check("world"));
    Ok(())
}
