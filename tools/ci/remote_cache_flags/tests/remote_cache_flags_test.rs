use remote_cache_flags::{Cache, flags};

fn cache<'a>(url: &'a str, header: &'a str, key: &'a str, event: &'a str) -> Cache<'a> {
    Cache {
        url,
        header,
        buildbuddy_key: key,
        event,
    }
}

#[test]
fn nothing_configured_prints_nothing() {
    assert!(flags(&cache("", "", "", "push")).unwrap().is_empty());
    assert!(flags(&cache("", "a=b", "", "push")).unwrap().is_empty());
}

#[test]
fn buildbuddy_shorthand_never_puts_the_key_on_the_command_line() {
    assert_eq!(
        flags(&cache("", "", "KEY", "pull_request")).unwrap(),
        ["--config=ci", "--config=ci-pr"]
    );
    assert_eq!(
        flags(&cache("", "", "KEY", "push")).unwrap(),
        ["--config=ci"]
    );
}

#[test]
fn url_and_header_become_flags() {
    assert_eq!(
        flags(&cache(
            "grpc://cache:9092",
            "Authorization=Basic abc",
            "",
            "push"
        ))
        .unwrap(),
        ["--config=ci", "--remote_cache=grpc://cache:9092"]
    );
}

#[test]
fn url_without_header_sends_none() {
    let list = flags(&cache("grpc://cache:9092", "", "", "pull_request")).unwrap();
    assert!(list.iter().all(|flag| !flag.starts_with("--remote_header")));
    assert!(list.contains(&"--config=ci-pr".to_string()));
}

#[test]
fn explicit_header_wins_over_the_shorthand() {
    let list = flags(&cache("", "x-api=1", "KEY", "push")).unwrap();
    assert_eq!(list, ["--config=ci"]);
}

#[test]
fn a_header_without_a_value_is_rejected() {
    for bad in ["novalue", "=v", "n="] {
        assert!(flags(&cache("grpc://c", bad, "", "push")).is_err(), "{bad}");
    }
}

#[test]
fn header_with_a_shell_character_is_rejected() {
    for header in ["a=b\"c", "a=b%c", "a=b&c", "a=b^c", "a\\b=c"] {
        assert!(
            flags(&cache("grpc://c", header, "", "push")).is_err(),
            "{header}"
        );
    }
}
