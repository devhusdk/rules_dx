#[must_use]
pub fn qualified_hosts() -> &'static [(&'static str, &'static str)] {
    &[
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "aarch64"),
        ("windows", "x86_64"),
        ("windows", "aarch64"),
    ]
}

#[must_use]
pub fn canonical_os(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "linux" => Some("linux"),
        "darwin" | "macos" => Some("macos"),
        "windows" | "win32" => Some("windows"),
        _ => None,
    }
}

#[must_use]
pub fn canonical_cpu(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "x86_64" | "x64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("arm64"),
        _ => None,
    }
}

#[must_use]
pub fn platform_key(os_raw: &str, cpu_raw: &str) -> Option<String> {
    match (canonical_os(os_raw), canonical_cpu(cpu_raw)) {
        (Some(os), Some(cpu)) => Some(format!("{os}_{cpu}")),
        _ => None,
    }
}

#[must_use]
pub fn supported_execution_keys() -> Vec<String> {
    qualified_hosts()
        .iter()
        .filter_map(|(os, arch)| platform_key(os, arch))
        .collect()
}

fn display_host(os: &str, arch: &str) -> String {
    let os = match os {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    };
    let cpu = canonical_cpu(arch).unwrap_or(arch);
    format!("{os} {cpu}")
}

fn qualified_host_list() -> String {
    let names: Vec<String> = qualified_hosts()
        .iter()
        .map(|(os, arch)| display_host(os, arch))
        .collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{}, and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

#[must_use]
pub fn refusal(os: &str, arch: &str) -> Option<String> {
    match platform_key(os, arch) {
        Some(key) if supported_execution_keys().contains(&key) => None,
        _ => Some(format!(
            "{os}/{arch} is not a supported host; dx runs on {} (unsupported_platform)",
            qualified_host_list()
        )),
    }
}

/// Refuses a host dx does not run on; Rosetta counts as the arm64 Mac it runs on.
#[must_use]
pub fn host_refusal() -> Option<String> {
    let (os, arch) = dx_shim::protocol::host_platform();
    refusal(os, arch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_shim::protocol::effective_arch;

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
        for (os, arch) in [("linux", "riscv64"), ("macos", "x86_64")] {
            let message = refusal(os, arch).expect("unqualified host must be refused");
            assert!(message.ends_with("(unsupported_platform)"), "{message}");
            assert!(message.contains(&format!("{os}/{arch}")), "{message}");
        }
    }

    #[test]
    fn qualified_set_matches_released_hosts() {
        assert_eq!(
            qualified_hosts(),
            &[
                ("linux", "x86_64"),
                ("linux", "aarch64"),
                ("macos", "aarch64"),
                ("windows", "x86_64"),
                ("windows", "aarch64")
            ]
        );
    }

    #[test]
    fn windows_arm64_host_is_qualified() {
        assert_eq!(refusal("windows", "aarch64"), None);
    }

    #[test]
    fn refusal_names_every_qualified_host() {
        let message = refusal("linux", "riscv64").expect("riscv64 must be refused");
        assert!(
            message.ends_with(
                "dx runs on Linux x86_64, Linux arm64, macOS arm64, Windows x86_64, and Windows arm64 (unsupported_platform)"
            ),
            "{message}"
        );
        for (os, arch) in qualified_hosts() {
            assert!(message.contains(&display_host(os, arch)), "{message}");
        }
    }

    #[test]
    fn adopt_status_accepts_every_qualified_host() {
        for (os, arch) in qualified_hosts() {
            assert_eq!(
                dx_adopt::status::host_check(os, arch).status,
                "ok",
                "dx status must accept qualified host {os}/{arch}"
            );
        }
    }

    #[test]
    fn macos_arm64_host_is_qualified() {
        assert_eq!(refusal("macos", "aarch64"), None);
    }

    #[test]
    fn macos_x86_64_is_refused_not_planned() {
        let message = refusal("macos", "x86_64").expect("macOS x86_64 must be refused");
        assert!(message.ends_with("(unsupported_platform)"), "{message}");
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
                "windows_x86_64".to_owned(),
                "windows_arm64".to_owned()
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

    fn sysctl_with<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            set.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn rosetta_on_apple_silicon_is_accepted() {
        for set in [
            &[("hw.optional.arm64", "1\n")][..],
            &[("hw.optional.arm64", "0"), ("sysctl.proc_translated", "1")][..],
        ] {
            let arch = effective_arch("macos", "x86_64", &sysctl_with(set));
            assert_eq!(arch, "aarch64");
            assert_eq!(refusal("macos", arch), None);
        }
    }

    #[test]
    fn intel_mac_is_still_refused() {
        for set in [
            &[][..],
            &[("hw.optional.arm64", "0"), ("sysctl.proc_translated", "0")][..],
        ] {
            let arch = effective_arch("macos", "x86_64", &sysctl_with(set));
            assert_eq!(arch, "x86_64");
            assert!(refusal("macos", arch).is_some());
        }
    }

    #[test]
    fn other_hosts_ignore_sysctl() {
        let never = |key: &str| -> Option<String> { panic!("sysctl {key} probed") };
        for (os, arch) in [
            ("linux", "x86_64"),
            ("linux", "aarch64"),
            ("windows", "x86_64"),
            ("macos", "aarch64"),
        ] {
            assert_eq!(effective_arch(os, arch, &never), arch);
        }
    }

    #[test]
    fn host_refusal_agrees_with_host_refusal_contract() {
        assert_eq!(
            host_refusal(),
            refusal(
                std::env::consts::OS,
                effective_arch(std::env::consts::OS, std::env::consts::ARCH, &read_sysctl)
            )
        );
    }
}
