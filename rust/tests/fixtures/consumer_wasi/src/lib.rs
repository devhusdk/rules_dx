pub const BUILD_TARGET: &str = env!("DX_WASI_BUILD_TARGET");
pub const BUILD_SCRIPT_FAMILY: &str = env!("DX_WASI_BUILD_SCRIPT_FAMILY");
pub const PROC_MACRO_FAMILY: &str = consumer_wasi_macros::expansion_family!();

pub fn module_family() -> &'static str {
    if cfg!(target_family = "wasm") {
        "wasm"
    } else {
        "native"
    }
}

pub fn env_or_unset(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "<unset>".to_owned())
}

pub fn read_or_no_access(path: &str) -> String {
    std::fs::read_to_string(path)
        .map(|content| content.trim_end_matches(['\r', '\n']).to_owned())
        .unwrap_or_else(|_| "no-access".to_owned())
}

pub fn report(argv: &[String]) -> Vec<String> {
    let hostile = if std::env::var("DX_WASI_HOSTILE").is_ok() {
        "leaked"
    } else {
        "hidden"
    };
    let probe = std::env::var("DX_WASI_PROBE").unwrap_or_else(|_| "/denied/secret.txt".to_owned());
    vec![
        format!("argv={argv:?}"),
        format!("declared={}", env_or_unset("DX_WASI_DECLARED")),
        format!("wrapped={}", env_or_unset("DX_WASI_WRAPPED")),
        format!("hostile={hostile}"),
        format!("data={}", read_or_no_access("/data/input.txt")),
        format!("probe={}", read_or_no_access(&probe)),
        format!("module={}", module_family()),
        format!("build_target={BUILD_TARGET}"),
        format!("build_script={BUILD_SCRIPT_FAMILY}"),
        format!("proc_macro={PROC_MACRO_FAMILY}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_build_keeps_host_tools_and_host_target() {
        assert_eq!(module_family(), "native");
        assert_eq!(BUILD_SCRIPT_FAMILY, "native");
        assert_eq!(PROC_MACRO_FAMILY, "native");
        assert!(!BUILD_TARGET.starts_with("wasm32"), "{BUILD_TARGET}");
    }

    #[test]
    fn report_marks_missing_capabilities() {
        let lines = report(&["one".to_owned()]);
        assert_eq!(lines[0], r#"argv=["one"]"#);
        assert_eq!(lines[1], "declared=<unset>");
        assert_eq!(lines[5], "probe=no-access");
        assert_eq!(lines[6], "module=native");
    }
}
