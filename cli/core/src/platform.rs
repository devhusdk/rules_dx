pub(crate) fn qualified_hosts() -> &'static [(&'static str, &'static str)] {
    &[
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "aarch64"),
        ("windows", "x86_64"),
    ]
}

pub(crate) fn canonical_os(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "linux" => Some("linux"),
        "darwin" | "macos" => Some("macos"),
        "windows" | "win32" => Some("windows"),
        _ => None,
    }
}

pub(crate) fn canonical_cpu(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "x86_64" | "x64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("arm64"),
        _ => None,
    }
}

pub(crate) fn platform_key(os_raw: &str, cpu_raw: &str) -> Option<String> {
    match (canonical_os(os_raw), canonical_cpu(cpu_raw)) {
        (Some(os), Some(cpu)) => Some(format!("{os}_{cpu}")),
        _ => None,
    }
}

pub(crate) fn supported_execution_keys() -> Vec<String> {
    qualified_hosts()
        .iter()
        .filter_map(|(os, arch)| platform_key(os, arch))
        .collect()
}

pub(crate) fn refusal(os: &str, arch: &str) -> Option<String> {
    match platform_key(os, arch) {
        Some(key) if supported_execution_keys().contains(&key) => None,
        _ => Some(format!(
            "unsupported_platform: {os}/{arch} is not a supported host; dx runs on Linux x86_64, Linux arm64, macOS arm64, and Windows x86_64"
        )),
    }
}

pub fn host_refusal() -> Option<String> {
    refusal(std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_host_is_qualified() {
        assert_eq!(refusal("linux", "x86_64"), None);
    }

    #[test]
    fn arm64_host_is_qualified() {
        assert_eq!(refusal("linux", "aarch64"), None);
    }

    #[test]
    fn unqualified_hosts_are_refused_with_pointer() {
        for (os, arch) in [("windows", "aarch64"), ("macos", "x86_64")] {
            let message = refusal(os, arch).expect("unqualified host must be refused");
            assert!(message.starts_with("unsupported_platform"), "{message}");
            assert!(message.contains(&format!("{os}/{arch}")), "{message}");
        }
    }

    #[test]
    fn qualified_set_is_seed_plus_arm64_plus_macos_plus_windows() {
        assert_eq!(
            qualified_hosts(),
            &[
                ("linux", "x86_64"),
                ("linux", "aarch64"),
                ("macos", "aarch64"),
                ("windows", "x86_64")
            ]
        );
    }

    #[test]
    fn macos_arm64_host_is_qualified() {
        assert_eq!(refusal("macos", "aarch64"), None);
    }

    #[test]
    fn macos_x86_64_is_refused_not_planned() {
        let message = refusal("macos", "x86_64").expect("macOS x86_64 must be refused");
        assert!(message.starts_with("unsupported_platform"), "{message}");
        assert!(message.contains("macos/x86_64"), "{message}");
    }

    #[test]
    fn windows_x86_64_host_is_qualified() {
        assert_eq!(refusal("windows", "x86_64"), None);
    }

    #[test]
    fn canonical_mapping_accepts_upstream_spellings() {
        assert_eq!(canonical_os("Linux"), Some("linux"));
        assert_eq!(canonical_os("Darwin"), Some("macos"));
        assert_eq!(canonical_os("macos"), Some("macos"));
        assert_eq!(canonical_os("windows"), Some("windows"));
        assert_eq!(canonical_os("win32"), Some("windows"));
        assert_eq!(canonical_cpu("AArch64"), Some("arm64"));
        assert_eq!(canonical_cpu("arm64"), Some("arm64"));
        assert_eq!(canonical_cpu("x86_64"), Some("x86_64"));
        assert_eq!(canonical_cpu("AMD64"), Some("x86_64"));
        assert_eq!(canonical_cpu("x64"), Some("x86_64"));
        assert_eq!(canonical_os("solaris"), None);
        assert_eq!(canonical_cpu("sparc"), None);
    }

    #[test]
    fn canonical_keys_match_supported_execution_set() {
        assert_eq!(
            supported_execution_keys(),
            vec![
                "linux_x86_64".to_owned(),
                "linux_arm64".to_owned(),
                "macos_arm64".to_owned(),
                "windows_x86_64".to_owned()
            ]
        );
        assert_eq!(
            platform_key("linux", "aarch64"),
            Some("linux_arm64".to_owned())
        );
        assert_eq!(
            platform_key("darwin", "arm64"),
            Some("macos_arm64".to_owned())
        );
        assert_eq!(platform_key("solaris", "sparc"), None);
    }

    #[test]
    fn host_refusal_agrees_with_host_refusal_contract() {
        assert_eq!(
            host_refusal(),
            refusal(std::env::consts::OS, std::env::consts::ARCH)
        );
    }
}
