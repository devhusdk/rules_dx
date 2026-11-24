use std::path::Path;

pub const SLOT_CLEAN: &str = "bazel clean in a slot drops its warm outputs and does not fix a stale cache. Rerun with --noremote_accept_cached, or release this slot and add a fresh one.";

pub const REFUSED_EXIT: i32 = 77;

const STARTUP_VALUES: &[&str] = &[
    "bazelrc",
    "command_port",
    "connect_timeout_secs",
    "digest_function",
    "experimental_cgroup_parent",
    "failure_detail_out",
    "host_jvm_args",
    "host_jvm_profile",
    "install_base",
    "install_md5",
    "invocation_policy",
    "io_nice_level",
    "local_startup_timeout_secs",
    "macos_qos_class",
    "max_idle_secs",
    "output_base",
    "output_user_root",
    "server_javabase",
    "server_jvm_out",
    "unix_digest_hash_attribute_name",
];

#[must_use]
pub fn command(args: &[String]) -> Option<&str> {
    let mut words = args.iter().map(String::as_str);
    while let Some(word) = words.next() {
        if let Some(option) = word.strip_prefix("--") {
            if STARTUP_VALUES.contains(&option) {
                words.next();
            }
        } else if !word.starts_with('-') {
            return Some(word);
        }
    }
    None
}

#[must_use]
pub fn refusal(dir: &Path, args: &[String]) -> Option<&'static str> {
    (command(args) == Some("clean") && super::lease_for_workspace(dir).is_some())
        .then_some(SLOT_CLEAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_bazel_command_skips_startup_options_and_their_values() {
        for (line, want) in [
            ("clean", Some("clean")),
            ("--output_base /x clean", Some("clean")),
            ("--output_base=/x clean --async", Some("clean")),
            ("--host_jvm_args -Xmx1g --batch clean", Some("clean")),
            ("--bazelrc a.rc --nohome_rc build //...", Some("build")),
            ("--batch", None),
            ("-h", None),
        ] {
            assert_eq!(command(&words(line)), want, "{line}");
        }
    }
}
