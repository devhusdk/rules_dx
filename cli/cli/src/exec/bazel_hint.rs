//! Specific next steps for the Bazel failures dx knows how to fix.

use super::failure::{
    CODE_FETCH_FAILED, CODE_LOCKFILE_DRIFT, CODE_MODULE_NOT_FOUND, CODE_NO_TOOLCHAIN,
    CODE_STALE_GENERATED,
};

/// One recognised failure: its stable error code and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hint {
    pub(crate) code: &'static str,
    pub(crate) next: String,
}

/// Classifies the stderr of a failed Bazel run; `None` means the generic hint applies.
pub(crate) fn classify(stderr: &str) -> Option<Hint> {
    module_not_found(stderr)
        .or_else(|| lockfile_drift(stderr))
        .or_else(|| no_toolchain(stderr))
        .or_else(|| stale_generated(stderr))
        .or_else(|| fetch_failed(stderr))
}

/// The hint for the Bazel run that just failed, when its stderr was captured.
pub(crate) fn recorded() -> Option<Hint> {
    classify(&dx_process::last_bazel_stderr())
}

/// The error code to report for a failed Bazel run whose generic code is `generic`.
pub(crate) fn refine(generic: &'static str) -> &'static str {
    recorded().map_or(generic, |hint| hint.code)
}

fn lockfile_drift(stderr: &str) -> Option<Hint> {
    let drifted = stderr.contains("lockfile_mode=error")
        || stderr.contains("no longer up-to-date")
        || (stderr.contains("lockfile") && stderr.contains("out of date"));
    drifted.then(|| Hint {
        code: CODE_LOCKFILE_DRIFT,
        next: "next: MODULE.bazel.lock is out of date; run `dx update --apply` (or `bazel mod deps --lockfile_mode=update`) and run the command again.".to_owned(),
    })
}

fn module_not_found(stderr: &str) -> Option<Hint> {
    let line = stderr
        .lines()
        .find(|line| line.contains("not found in registries"))?;
    let before = line.split(": module ").nth(1)?.split(" not found").next()?;
    let (name, version) = before.split_once('@').unwrap_or((before, ""));
    let module = if version.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {version}")
    };
    Some(Hint {
        code: CODE_MODULE_NOT_FOUND,
        next: format!(
            "next: module {module} is not in the configured registries; check its name and version in MODULE.bazel and the --registry settings, then run the command again."
        ),
    })
}

fn stale_generated(stderr: &str) -> Option<Hint> {
    let line = stderr.lines().find(|line| {
        line.contains("no such package")
            && (line.contains("BUILD file not found") || line.contains(".dx/"))
    })?;
    let package = line
        .split("no such package '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())
        .unwrap_or("a generated label");
    Some(Hint {
        code: CODE_STALE_GENERATED,
        next: format!(
            "next: package {package} has no BUILD file, so generated files are missing or stale; run `dx generate --apply` and run the command again."
        ),
    })
}

fn no_toolchain(stderr: &str) -> Option<Hint> {
    let line = stderr.lines().find(|line| {
        line.contains("No matching toolchains found")
            || line.contains("is incompatible and cannot be built")
    })?;
    let subject = if let Some(types) = line.split("for types").nth(1) {
        format!(
            "for {}",
            types
                .trim_start_matches(':')
                .split_whitespace()
                .next()
                .unwrap_or("the requested type")
                .trim_end_matches(&[',', '.'][..])
        )
    } else {
        let target = line
            .split_whitespace()
            .find(|word| word.starts_with("//") || word.starts_with('@'))
            .unwrap_or("the target");
        format!("for {target}")
    };
    let platform = stderr
        .split_whitespace()
        .find(|word| word.contains("platforms//") || word.contains("//platforms"))
        .map(|word| {
            word.trim_matches(|c: char| matches!(c, '\'' | '"' | ',' | ':' | '.' | '(' | ')'))
        });
    let on = platform.map_or(String::new(), |platform| format!(" on platform {platform}"));
    Some(Hint {
        code: CODE_NO_TOOLCHAIN,
        next: format!(
            "next: no matching toolchain {subject}{on}; run `dx status` to see the toolchains and platforms dx pins, then run the command again."
        ),
    })
}

fn fetch_failed(stderr: &str) -> Option<Hint> {
    const SIGNS: &[&str] = &[
        "UnknownHostException",
        "SSLHandshakeException",
        "PKIX path building failed",
        "Unable to tunnel through proxy",
        "Connection refused",
        "Connect timed out",
        "Error downloading",
        "403 Forbidden",
        "404 Not Found",
        "GET returned 40",
    ];
    if !SIGNS.iter().any(|sign| stderr.contains(sign)) {
        return None;
    }
    let url = stderr
        .split(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '(' | ')' | '\'' | '"' | ','))
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .map(|word| word.trim_end_matches(&[':', '.'][..]));
    let named = url.map_or("a download".to_owned(), |url| url.to_owned());
    Some(Hint {
        code: CODE_FETCH_FAILED,
        next: format!(
            "next: Bazel could not fetch {named}; check the network and proxy settings (HTTPS_PROXY) and the URL, or point at a populated cache with `--repository_cache`, then run the command again."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(stderr: &str) -> Option<&'static str> {
        classify(stderr).map(|hint| hint.code)
    }

    #[test]
    fn lockfile_drift_points_at_dx_update() {
        let hint = classify(
            "ERROR: MODULE.bazel.lock is no longer up-to-date because: the root module file changed. Please run `bazel mod deps --lockfile_mode=update`",
        )
        .unwrap();
        assert_eq!(hint.code, "lockfile_drift");
        assert!(hint.next.contains("dx update --apply"), "{}", hint.next);
    }

    #[test]
    fn a_missing_package_points_at_dx_generate() {
        let hint = classify("ERROR: no such package 'gen/app': BUILD file not found in any of the following directories. Add a BUILD file").unwrap();
        assert_eq!(hint.code, "stale_generated");
        assert!(hint.next.contains("gen/app"), "{}", hint.next);
        assert!(hint.next.contains("dx generate --apply"), "{}", hint.next);
    }

    #[test]
    fn fetch_failures_name_the_url() {
        let hint = classify(
            "ERROR: Error downloading [https://example.com/a/b.tar.gz] to /x: Unknown host example.com\njava.net.UnknownHostException",
        )
        .unwrap();
        assert_eq!(hint.code, "fetch_failed");
        assert!(
            hint.next.contains("https://example.com/a/b.tar.gz"),
            "{}",
            hint.next
        );
        assert!(hint.next.contains("--repository_cache"), "{}", hint.next);
        assert_eq!(code("GET returned 403 Forbidden"), Some("fetch_failed"));
        assert_eq!(code("PKIX path building failed"), Some("fetch_failed"));
    }

    #[test]
    fn a_missing_toolchain_names_the_type_and_points_at_status() {
        let hint = classify(
            "ERROR: While resolving toolchains for target //a:b (abc): No matching toolchains found for types @@rules_cc//cc:toolchain_type. Platform @@platforms//os:windows",
        )
        .unwrap();
        assert_eq!(hint.code, "no_toolchain");
        assert!(
            hint.next.contains("@@rules_cc//cc:toolchain_type"),
            "{}",
            hint.next
        );
        assert!(hint.next.contains("dx status"), "{}", hint.next);
    }

    #[test]
    fn an_incompatible_target_names_the_target() {
        let hint = classify("ERROR: Target //a:b is incompatible and cannot be built, but was explicitly requested.").unwrap();
        assert_eq!(hint.code, "no_toolchain");
        assert!(hint.next.contains("//a:b"), "{}", hint.next);
    }

    #[test]
    fn a_missing_module_names_the_module_and_version() {
        let hint = classify(
            "ERROR: Error computing the main repository mapping: in module dependency chain <root> -> nonexistent_mod@1.0: module nonexistent_mod@1.0 not found in registries:\n* https://raw.githubusercontent.com/x/MODULE.bazel: not found",
        )
        .unwrap();
        assert_eq!(hint.code, "module_not_found");
        assert!(hint.next.contains("nonexistent_mod 1.0"), "{}", hint.next);
    }

    #[test]
    fn an_unrecognised_failure_gets_no_specific_hint() {
        assert_eq!(code("ERROR: //a:b: compilation failed"), None);
        assert_eq!(code(""), None);
    }
}
