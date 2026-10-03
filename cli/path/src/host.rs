//! Host facts about naming an executable.

/// Suffixes the host runs a file under.
pub const EXECUTABLE_SUFFIXES: &[&str] = &[".exe", ".bat", ".cmd", ".com"];

/// Names Windows reserves for devices.
pub const RESERVED_STEMS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Maps one logical tool name to its host-native filename.
pub fn host_filename(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// Reports whether one name carries an executable suffix.
pub fn has_executable_suffix(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXECUTABLE_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// Removes an executable suffix from one name.
pub fn strip_executable_suffix(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    for suffix in EXECUTABLE_SUFFIXES {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return &name[..name.len() - suffix.len()];
        }
    }
    name
}

/// Reports whether one name is a device Windows reserves.
pub fn is_reserved_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    RESERVED_STEMS.contains(&stem.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_name_carries_no_suffix() {
        assert!(has_executable_suffix("python.exe"));
        assert!(!has_executable_suffix("python"));
        assert!(has_executable_suffix("prettier.EXE"));
        assert!(has_executable_suffix("build.bat"));
        assert!(!has_executable_suffix("pydoclint"));
    }

    #[test]
    fn a_suffix_strips_back_to_the_tool_name() {
        assert_eq!(strip_executable_suffix("python.exe"), "python");
        assert_eq!(strip_executable_suffix("build.bat"), "build");
        assert_eq!(strip_executable_suffix("pydoclint"), "pydoclint");
        assert_eq!(strip_executable_suffix(".exe"), ".exe");
    }

    #[test]
    fn a_reserved_stem_is_rejected_whatever_the_suffix() {
        assert!(is_reserved_device("nul"));
        assert!(is_reserved_device("NUL.txt"));
        assert!(is_reserved_device("com1.log"));
        assert!(!is_reserved_device("console"));
        assert!(!is_reserved_device("pydoclint"));
    }

    #[test]
    fn the_host_filename_follows_the_host() {
        let expected = if cfg!(windows) {
            "python.exe"
        } else {
            "python"
        };
        assert_eq!(host_filename("python"), expected);
    }
}
