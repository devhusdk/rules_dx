use super::{AUDIT_TODAY_ENV, PolicyError, load_security_policy, today_utc};

#[test]
fn a_pinned_date_makes_the_audit_date_reproducible() {
    assert_eq!(today_utc(Some("2020-01-02")), "2020-01-02");
    assert_eq!(AUDIT_TODAY_ENV, "DX_AUDIT_TODAY");
}

#[test]
fn an_unusable_pin_falls_back_to_today() {
    for bad in [
        "",
        "2020-1-2",
        "2020-01-02T00:00",
        "not-a-date",
        "2020/01/02x",
    ] {
        let today = today_utc(None);
        assert_eq!(today_utc(Some(bad)), today, "{bad:?} is not a pinned date");
    }
}

#[test]
fn security_policy_read_failures_and_absent_file_are_explicit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path();
    let absent = load_security_policy(workspace).expect("absent file");
    assert!(absent.exceptions.is_empty(), "{:?}", absent.exceptions);
    let path = workspace.join("security.toml");
    std::fs::write(&path, " \n").expect("empty");
    let empty = load_security_policy(workspace).expect("empty file");
    assert!(empty.exceptions.is_empty(), "{:?}", empty.exceptions);
    std::fs::write(&path, [0xff]).expect("invalid utf8");
    assert!(matches!(
        load_security_policy(workspace),
        Err(PolicyError::NotUtf8 { .. })
    ));
    std::fs::remove_file(&path).expect("remove");
    std::fs::create_dir_all(&path).expect("directory");
    assert!(matches!(
        load_security_policy(workspace),
        Err(PolicyError::Read { .. })
    ));
}

#[test]
fn an_invalid_dx_toml_fails_the_security_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("dx.toml"), "tier = \"strict\"\n").expect("dx.toml");
    let security = load_security_policy(dir.path()).expect_err("security");
    assert!(matches!(security, PolicyError::SecurityInvalid(_)));
    assert!(
        security.to_string().contains("dx.toml: ")
            && security.to_string().contains("schema_version is missing"),
        "{security}"
    );
}
