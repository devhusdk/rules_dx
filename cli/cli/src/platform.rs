pub fn qualified_hosts() -> &'static [(&'static str, &'static str)] {
    &[
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "aarch64"),
        ("windows", "x86_64"),
    ]
}

pub fn canonical_os(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "linux" => Some("linux"),
        "darwin" | "macos" => Some("macos"),
        "windows" | "win32" => Some("windows"),
        _ => None,
    }
}

pub fn canonical_cpu(raw: &str) -> Option<&'static str> {
    let lowered = raw.to_lowercase();
    match lowered.as_str() {
        "x86_64" | "x64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("arm64"),
        _ => None,
    }
}

pub fn platform_key(os_raw: &str, cpu_raw: &str) -> Option<String> {
    match (canonical_os(os_raw), canonical_cpu(cpu_raw)) {
        (Some(os), Some(cpu)) => Some(format!("{os}_{cpu}")),
        _ => None,
    }
}

pub fn supported_execution_keys() -> Vec<String> {
    qualified_hosts()
        .iter()
        .filter_map(|(os, arch)| platform_key(os, arch))
        .collect()
}

pub fn platform_role_error(role: &str, os_raw: &str, cpu_raw: &str) -> Option<String> {
    if platform_key(os_raw, cpu_raw).is_some() {
        None
    } else {
        Some(format!(
            "{role} platform: unknown os/cpu '{os_raw}/{cpu_raw}'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"
        ))
    }
}

pub struct HostPlatform {
    pub os: &'static str,
    pub cpu: &'static str,
}

impl HostPlatform {
    pub fn current() -> Option<Self> {
        match (
            canonical_os(std::env::consts::OS),
            canonical_cpu(std::env::consts::ARCH),
        ) {
            (Some(os), Some(cpu)) => Some(Self { os, cpu }),
            _ => None,
        }
    }

    pub fn key(&self) -> String {
        format!("{}_{}", self.os, self.cpu)
    }

    pub fn current_key() -> String {
        match Self::current() {
            Some(host) => host.key(),
            None => format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

#[derive(Debug)]
pub struct ExecutionPlatform {
    pub key: String,
}

impl ExecutionPlatform {
    pub fn role() -> &'static str {
        "execution"
    }

    pub fn parse(key: &str) -> Result<Self, String> {
        if supported_execution_keys().contains(&key.to_owned()) {
            Ok(Self {
                key: key.to_owned(),
            })
        } else {
            Err(format!(
                "unknown_execution_platform: {key} is not a supported execution platform; tools resolve for linux_x86_64, linux_arm64, macos_arm64, windows_x86_64"
            ))
        }
    }

    pub fn from_raw(os_raw: &str, cpu_raw: &str) -> Result<Self, String> {
        match platform_key(os_raw, cpu_raw) {
            Some(key) => Self::parse(&key),
            None => Err(platform_role_error(Self::role(), os_raw, cpu_raw)
                .unwrap_or_else(|| format!("unknown_execution_platform: {os_raw}/{cpu_raw}"))),
        }
    }
}

#[derive(Debug)]
pub struct TargetPlatform {
    pub key: String,
}

impl TargetPlatform {
    pub fn role() -> &'static str {
        "target"
    }

    pub fn parse(key: &str) -> Result<Self, String> {
        if ExecutionPlatform::parse(key).is_ok() {
            Ok(Self {
                key: key.to_owned(),
            })
        } else {
            Err(format!(
                "unknown_target_platform: {key} is not a supported target platform; want one of linux_x86_64, linux_arm64, macos_arm64, windows_x86_64"
            ))
        }
    }

    pub fn from_raw(os_raw: &str, cpu_raw: &str) -> Result<Self, String> {
        match platform_key(os_raw, cpu_raw) {
            Some(key) => Self::parse(&key),
            None => Err(platform_role_error(Self::role(), os_raw, cpu_raw)
                .unwrap_or_else(|| format!("unknown_target_platform: {os_raw}/{cpu_raw}"))),
        }
    }
}

pub struct ExecutionAssignment {
    pub host: String,
    pub execution: String,
}

impl ExecutionAssignment {
    pub fn new(host: &str, execution: &str) -> Self {
        Self {
            host: host.to_owned(),
            execution: execution.to_owned(),
        }
    }

    pub fn describe(&self) -> String {
        format!("host {} runs tools built for {}", self.host, self.execution)
    }
}

pub fn refusal(os: &str, arch: &str) -> Option<String> {
    match platform_key(os, arch) {
        Some(key) if supported_execution_keys().contains(&key) => None,
        _ => Some(format!(
            "unsupported_platform: {os}/{arch} is not a supported host; dx runs on Linux x86_64, Linux arm64, macOS arm64, and Windows x86_64"
        )),
    }
}

pub fn host_refusal() -> Option<String> {
    match HostPlatform::current() {
        Some(host) if supported_execution_keys().contains(&host.key()) => None,
        _ => refusal(std::env::consts::OS, std::env::consts::ARCH),
    }
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
    fn role_errors_name_the_role_without_fallback() {
        assert_eq!(platform_role_error("execution", "linux", "aarch64"), None);
        assert_eq!(
            platform_role_error("execution", "solaris", "sparc"),
            Some("execution platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)".to_owned())
        );
        assert_eq!(
            platform_role_error("host", "plan9", "mips"),
            Some("host platform: unknown os/cpu 'plan9/mips'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)".to_owned())
        );
        assert_eq!(
            platform_role_error("target", "fuchsia", "riscv64"),
            Some("target platform: unknown os/cpu 'fuchsia/riscv64'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)".to_owned())
        );
    }

    #[test]
    fn execution_and_target_parse_known_keys() {
        assert_eq!(ExecutionPlatform::role(), "execution");
        assert_eq!(TargetPlatform::role(), "target");
        let execution = ExecutionPlatform::parse("linux_arm64").expect("known execution key");
        assert_eq!(execution.key, "linux_arm64");
        let target = TargetPlatform::parse("windows_x86_64").expect("known target key");
        assert_eq!(target.key, "windows_x86_64");
        assert_eq!(
            ExecutionPlatform::from_raw("linux", "aarch64")
                .expect("raw execution")
                .key,
            "linux_arm64"
        );
        assert_eq!(
            TargetPlatform::from_raw("darwin", "arm64")
                .expect("raw target")
                .key,
            "macos_arm64"
        );
    }

    #[test]
    fn unknown_execution_and_target_keys_fail_explicitly() {
        let error = ExecutionPlatform::parse("solaris_sparc").expect_err("unknown execution key");
        assert!(error.starts_with("unknown_execution_platform"), "{error}");
        assert!(error.contains("solaris_sparc"), "{error}");
        let error = ExecutionPlatform::parse("macos_x86_64").expect_err("unclaimed execution key");
        assert!(error.starts_with("unknown_execution_platform"), "{error}");
        let error = TargetPlatform::parse("windows_arm64").expect_err("unclaimed target key");
        assert!(error.starts_with("unknown_target_platform"), "{error}");
        assert!(error.contains("windows_arm64"), "{error}");
        let error = ExecutionPlatform::from_raw("solaris", "sparc").expect_err("unknown raw");
        assert_eq!(
            error,
            "execution platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"
        );
        let error = TargetPlatform::from_raw("solaris", "sparc").expect_err("unknown raw");
        assert_eq!(
            error,
            "target platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"
        );
    }

    #[test]
    fn remote_execution_differs_from_the_cli_host() {
        let assignment = ExecutionAssignment::new("linux_x86_64", "windows_x86_64");
        assert_eq!(
            assignment.describe(),
            "host linux_x86_64 runs tools built for windows_x86_64"
        );
        let local =
            ExecutionAssignment::new(&HostPlatform::current_key(), &HostPlatform::current_key());
        assert!(
            local.describe().starts_with("host "),
            "{}",
            local.describe()
        );
    }

    #[test]
    fn host_refusal_agrees_with_host_refusal_contract() {
        assert_eq!(
            host_refusal(),
            refusal(std::env::consts::OS, std::env::consts::ARCH)
        );
    }
}
