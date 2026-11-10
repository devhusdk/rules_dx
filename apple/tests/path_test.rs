use dx_testing::resolve_runfiles;

#[test]
fn no_apple_action_puts_a_host_directory_on_path() {
    let sources = std::env::var("DX_APPLE_PATH_SOURCES").unwrap_or_default();
    let mut seen = 0;
    for entry in sources.split_whitespace() {
        let path = resolve_runfiles(entry);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for host in ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"] {
            assert!(!text.contains(host), "{entry} names host directory {host}");
        }
        seen += 1;
    }
    assert_eq!(seen, 4, "{sources}");
}
