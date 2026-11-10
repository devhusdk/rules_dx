// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() -> anyhow::Result<()> {
    let greeting = hello::greet("world")?;
    println!("{greeting}");
    Ok(())
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
