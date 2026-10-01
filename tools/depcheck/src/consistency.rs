use std::collections::BTreeMap;
use std::path::Path;

use crate::ecosystem::{cc, dotnet, go, js, jvm, python, ruby, rust};
use crate::version::satisfies_for;
use crate::{DepInfo, DepcheckError, Ecosystem, WorkspaceLocks};

pub(crate) fn load_manifest(
    eco: Ecosystem,
    manifest: &Path,
) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    match eco {
        Ecosystem::Rust => rust::parse_rust_manifest(manifest),
        Ecosystem::Python => python::parse_python_manifest(manifest),
        Ecosystem::Js | Ecosystem::Ts => js::parse_js_manifest(manifest),
        Ecosystem::Go => go::parse_go_manifest(manifest),
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => jvm::parse_jvm_manifest(manifest),
        Ecosystem::Csharp | Ecosystem::Fsharp => dotnet::parse_dotnet_manifest(manifest),
        Ecosystem::Cc => cc::parse_cc_manifest(manifest),
        Ecosystem::Ruby => ruby::parse_ruby_manifest(manifest),
    }
}

fn load_lock(eco: Ecosystem, lock: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    match eco {
        Ecosystem::Rust => rust::parse_rust_lock(lock),
        Ecosystem::Python => python::parse_python_lock(lock),
        Ecosystem::Js | Ecosystem::Ts => js::parse_pnpm_lock(lock),
        Ecosystem::Go => go::parse_go_lock(lock),
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => jvm::parse_jvm_lock(lock),
        Ecosystem::Csharp | Ecosystem::Fsharp => dotnet::parse_dotnet_lock(lock),
        Ecosystem::Cc => cc::parse_cc_lock(lock),
        Ecosystem::Ruby => ruby::parse_ruby_lock(lock),
    }
}

pub fn cmd_consistency(
    eco: Ecosystem,
    manifest: &Path,
    lock: &Path,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    if !manifest.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: manifest missing: {}",
            manifest.display()
        );
        return 2;
    }
    if !lock.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: lock missing: {} (declare lock inputs, do not skip)",
            lock.display()
        );
        return 2;
    }
    let mut deps = match load_manifest(eco, manifest) {
        Ok(deps) => deps,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let pkgs = match load_lock(eco, lock) {
        Ok(pkgs) => pkgs,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    let cc_lock_sha = if eco == Ecosystem::Cc {
        cc::parse_cc_lock_sha(lock)
    } else {
        BTreeMap::new()
    };
    check_maps(eco, &deps, &pkgs, &cc_lock_sha, stdout, stderr)
}

fn check_maps(
    eco: Ecosystem,
    deps: &BTreeMap<String, DepInfo>,
    pkgs: &BTreeMap<String, String>,
    cc_lock_sha: &BTreeMap<String, String>,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    let mut failures = Vec::new();
    for (name, info) in deps {
        let mut locked = pkgs.get(name).cloned();
        if locked.is_none() {
            let alt = if name.contains('_') {
                name.replace('_', "-")
            } else {
                name.replace('-', "_")
            };
            locked = pkgs.get(&alt).cloned();
        }
        let Some(locked) = locked else {
            failures.push(format!(
                "missing lock entry for '{}' (manifest requires {})",
                info.raw, info.spec
            ));
            continue;
        };
        if !satisfies_for(eco, &info.spec, &locked) {
            failures.push(format!(
                "stale lock entry for '{}': manifest requires {}, lock has {locked}",
                info.raw, info.spec
            ));
        } else if eco == Ecosystem::Cc {
            let want_sha = info.sha256.trim().to_owned();
            let got_sha = cc_lock_sha
                .get(name)
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_owned();
            if want_sha.is_empty() {
                failures.push(format!(
                    "missing sha256 for '{}' (every http_archive carries sha256/integrity)",
                    info.raw
                ));
            } else if want_sha != got_sha {
                failures.push(format!(
                    "stale sha256 for '{}': manifest has {want_sha}, lock has {got_sha}",
                    info.raw
                ));
            }
        }
    }
    if !failures.is_empty() {
        for failure in &failures {
            let _ = writeln!(stderr, "depcheck: FAIL: consistency: {failure}");
        }
        return 1;
    }
    let _ = writeln!(
        stdout,
        "depcheck: OK: consistency: {} declarations match lock ({})",
        deps.len(),
        eco.name()
    );
    0
}

fn check_pair(
    eco: Ecosystem,
    manifest: &Path,
    lock: &Path,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    if !manifest.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: manifest missing: {}",
            manifest.display()
        );
        return 2;
    }
    if !lock.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: lock missing: {} (declare lock inputs, do not skip)",
            lock.display()
        );
        return 2;
    }
    let mut deps = match load_manifest(eco, manifest) {
        Ok(deps) => deps,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let pkgs = match load_lock(eco, lock) {
        Ok(pkgs) => pkgs,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    check_maps(eco, &deps, &pkgs, &BTreeMap::new(), stdout, stderr)
}

fn check_maven_pair(
    artifacts: &Path,
    lock: &Path,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    if !artifacts.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: manifest missing: {}",
            artifacts.display()
        );
        return 2;
    }
    if !lock.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: lock missing: {} (declare lock inputs, do not skip)",
            lock.display()
        );
        return 2;
    }
    let deps = match jvm::parse_maven_artifacts_list(artifacts) {
        Ok(deps) => deps,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    let pkgs = match jvm::parse_jvm_lock(lock) {
        Ok(pkgs) => pkgs,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    check_maps(
        Ecosystem::Java,
        &deps,
        &pkgs,
        &BTreeMap::new(),
        stdout,
        stderr,
    )
}

pub fn cmd_locks(
    locks: &WorkspaceLocks,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    let mut worst = 0;
    for code in [
        check_pair(
            Ecosystem::Rust,
            locks.cargo_manifest,
            locks.cargo_lock,
            stdout,
            stderr,
        ),
        check_pair(
            Ecosystem::Python,
            locks.uv_manifest,
            locks.uv_lock,
            stdout,
            stderr,
        ),
        check_pair(
            Ecosystem::Js,
            locks.pnpm_manifest,
            locks.pnpm_lock,
            stdout,
            stderr,
        ),
        check_pair(
            Ecosystem::Go,
            locks.go_manifest,
            locks.go_lock,
            stdout,
            stderr,
        ),
        check_maven_pair(locks.maven_artifacts, locks.maven_lock, stdout, stderr),
        check_pair(
            Ecosystem::Csharp,
            locks.paket_manifest,
            locks.paket_lock,
            stdout,
            stderr,
        ),
        check_pair(
            Ecosystem::Ruby,
            locks.ruby_manifest,
            locks.ruby_lock,
            stdout,
            stderr,
        ),
    ] {
        worst = worst.max(code);
    }
    worst
}
