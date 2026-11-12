//! cpansa_dump prints the CPANSA-DB version line, then the database as a JSON object.

use dx_testing::{env_file, run};

#[test]
fn prints_version_then_database() {
    let out = run(&env_file("CPANSA_DUMP"), &[], &[]).expect("cpansa_dump runs");
    assert!(out.status.success(), "{}", out.combined());
    let (version, database) = out
        .stdout
        .split_once('\n')
        .unwrap_or_else(|| panic!("no version line:\n{}", out.combined()));
    assert!(
        version.starts_with(|c: char| c.is_ascii_digit()),
        "bad version line: {version}"
    );
    let database = database.trim();
    assert!(
        database.starts_with('{') && database.ends_with('}'),
        "database is not a JSON object: {}",
        database.chars().take(80).collect::<String>()
    );
}
