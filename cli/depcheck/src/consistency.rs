use std::collections::BTreeMap;
use std::path::Path;

use crate::ecosystem::{
    beam, cc, dart, dlang, dotnet, fortran, go, haskell, js, julia, jvm, lua, ocaml,
    parse_paren_lock, perl, php, powershell, python, ruby, rust, swift, zig,
};
use dx_error::DxError;

use crate::version::satisfies_for;
use crate::{DepInfo, DepcheckError, LockVersions, WorkspaceLocks};
use dx_ecosystems::Language;

pub(crate) fn load_manifest(
    eco: Language,
    manifest: &Path,
) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    match eco {
        Language::Rust => rust::parse_rust_manifest(manifest),
        Language::Python => python::parse_python_manifest(manifest),
        Language::Js | Language::Ts => js::parse_js_manifest(manifest),
        Language::Go => go::parse_go_manifest(manifest),
        Language::Java | Language::Kotlin | Language::Scala => jvm::parse_jvm_manifest(manifest),
        Language::Csharp | Language::Fsharp => dotnet::parse_dotnet_manifest(manifest),
        Language::Cc => cc::parse_cc_manifest(manifest),
        Language::Ruby => ruby::parse_ruby_manifest(manifest),
        Language::Perl => perl::parse_perl_manifest(manifest),
        Language::Zig => zig::parse_zig_manifest(manifest),
        Language::Julia => julia::parse_julia_manifest(manifest),
        Language::Dart => dart::parse_dart_manifest(manifest),
        Language::Php => php::parse_php_manifest(manifest),
        Language::Elixir => beam::parse_elixir_manifest(manifest),
        Language::Erlang => beam::parse_erlang_manifest(manifest),
        Language::Fortran => fortran::parse_fortran_manifest(manifest),
        Language::D => dlang::parse_d_manifest(manifest),
        Language::Haskell => haskell::parse_haskell_manifest(manifest),
        Language::Swift => swift::parse_swift_manifest(manifest),
        Language::Powershell => powershell::parse_powershell_manifest(manifest),
        Language::Lua => lua::parse_lua_manifest(manifest),
        Language::Ocaml => ocaml::parse_ocaml_manifest(manifest),
    }
}

fn single(
    pkgs: Result<BTreeMap<String, String>, DepcheckError>,
) -> Result<LockVersions, DepcheckError> {
    Ok(pkgs?.into_iter().map(|(k, v)| (k, vec![v])).collect())
}

fn load_lock(eco: Language, lock: &Path) -> Result<LockVersions, DepcheckError> {
    match eco {
        Language::Rust => rust::parse_rust_lock(lock),
        Language::Python => python::parse_python_lock(lock),
        Language::Js | Language::Ts => Err(DepcheckError::LockParse(
            "pnpm locks are read per importer".to_owned(),
        )),
        Language::Go => single(go::parse_go_lock(lock)),
        Language::Java | Language::Kotlin | Language::Scala => single(jvm::parse_jvm_lock(lock)),
        Language::Csharp | Language::Fsharp => single(parse_paren_lock(lock)),
        Language::Cc => single(cc::parse_cc_lock(lock)),
        Language::Ruby => single(parse_paren_lock(lock)),
        Language::Perl => single(perl::parse_perl_lock(lock)),
        Language::Zig => single(zig::parse_zig_lock(lock)),
        Language::Julia => single(julia::parse_julia_lock(lock)),
        Language::Dart => dart::parse_dart_lock(lock),
        Language::Php => single(php::parse_php_lock(lock)),
        Language::Elixir => single(beam::parse_mix_lock(lock)),
        Language::Erlang => single(beam::parse_rebar_lock(lock)),
        Language::Fortran => single(fortran::parse_fortran_lock(lock)),
        Language::D => single(dlang::parse_d_lock(lock)),
        Language::Haskell => single(haskell::parse_haskell_lock(lock)),
        Language::Swift => single(swift::parse_swift_lock(lock)),
        Language::Powershell => single(powershell::parse_powershell_lock(lock)),
        Language::Lua => single(lua::parse_lua_lock(lock)),
        Language::Ocaml => single(ocaml::parse_ocaml_lock(lock)),
    }
}

type DepLoader = fn(Language, &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError>;
type PkgLoader = fn(Language, &Path) -> Result<LockVersions, DepcheckError>;

/// One manifest/lock pair to check, with the parsers that read it.
pub(crate) struct Pair<'a> {
    eco: Language,
    manifest: &'a Path,
    lock: &'a Path,
    load_deps: DepLoader,
    load_pkgs: PkgLoader,
}

/// Builds the pair an ecosystem's manifest and lock parsers read.
pub(crate) fn lock_pair<'a>(eco: Language, manifest: &'a Path, lock: &'a Path) -> Pair<'a> {
    Pair {
        eco,
        manifest,
        lock,
        load_deps: load_manifest,
        load_pkgs: load_lock,
    }
}

pub fn cmd_consistency(
    eco: Language,
    manifest: &Path,
    lock: &Path,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    check_pair(lock_pair(eco, manifest, lock), stdout, stderr)
}

fn check_maps(
    eco: Language,
    deps: &BTreeMap<String, DepInfo>,
    pkgs: &LockVersions,
    cc_lock_sha: &BTreeMap<String, String>,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    let mut failures = Vec::new();
    for (name, info) in deps {
        let key = if info.package.is_empty() {
            name
        } else {
            &info.package
        };
        let alt = if key.contains('_') {
            key.replace('_', "-")
        } else {
            key.replace('-', "_")
        };
        let locked = pkgs
            .get(key)
            .filter(|v| !v.is_empty())
            .or_else(|| pkgs.get(&alt).filter(|v| !v.is_empty()));
        let Some(locked) = locked else {
            failures.push(format!(
                "missing lock entry for '{}' (manifest requires {})",
                info.raw, info.spec
            ));
            continue;
        };
        if !locked.iter().any(|v| satisfies_for(eco, &info.spec, v)) {
            failures.push(format!(
                "stale lock entry for '{}': manifest requires {}, lock has {}",
                info.raw,
                info.spec,
                locked.join(", ")
            ));
        } else if eco == Language::Zig {
            let want = info.sha256.trim();
            if want.is_empty() {
                failures.push(format!("missing hash for '{}'", info.raw));
            } else if !locked.iter().any(|v| v.trim() == want) {
                failures.push(format!(
                    "stale hash for '{}': manifest has {want}, lock has {}",
                    info.raw,
                    locked.join(", ")
                ));
            }
        } else if eco == Language::Cc {
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
    report(eco, deps.len(), &failures, stdout, stderr)
}

fn report(
    eco: Language,
    declared: usize,
    failures: &[String],
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    if !failures.is_empty() {
        for failure in failures {
            let _ = writeln!(stderr, "depcheck: FAIL: consistency: {failure}");
        }
        return Err(DxError::failure(format!(
            "{} consistency failures ({})",
            failures.len(),
            eco.name()
        )));
    }
    let _ = writeln!(
        stdout,
        "depcheck: OK: consistency: {declared} declarations match lock ({})",
        eco.name()
    );
    Ok(())
}

pub(crate) fn reported(stderr: &mut dyn std::fmt::Write, error: DxError) -> DxError {
    let _ = writeln!(stderr, "depcheck: ERROR: {error}");
    error
}

fn check_pnpm(
    deps: &BTreeMap<String, DepInfo>,
    manifest: &Path,
    lock_path: &Path,
) -> Result<Vec<String>, DepcheckError> {
    let key = js::pnpm_importer_key(manifest, lock_path)?;
    let lock = js::parse_pnpm_lock(lock_path)?;
    let Some(importer) = lock.importer(&key) else {
        return Err(DepcheckError::LockParse(format!(
            "no importer '{key}' (lock has: {})",
            lock.importer_keys().join(", ")
        )));
    };
    js::pnpm_failures(deps, importer, &lock)
}

pub(crate) fn check_pair(
    pair: Pair<'_>,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    if !pair.manifest.exists() {
        return Err(reported(
            stderr,
            DxError::usage(format!("manifest missing: {}", pair.manifest.display())),
        ));
    }
    if !pair.lock.exists() {
        return Err(reported(
            stderr,
            DxError::usage(format!(
                "lock missing: {} (declare lock inputs, do not skip)",
                pair.lock.display()
            )),
        ));
    }
    let mut deps = match (pair.load_deps)(pair.eco, pair.manifest) {
        Ok(deps) => deps,
        Err(err) => {
            return Err(reported(stderr, err.into()));
        }
    };
    if matches!(pair.eco, Language::Js | Language::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    if matches!(pair.eco, Language::Js | Language::Ts) {
        return match check_pnpm(&deps, pair.manifest, pair.lock) {
            Ok(failures) => report(pair.eco, deps.len(), &failures, stdout, stderr),
            Err(err) => Err(reported(stderr, err.into())),
        };
    }
    let pkgs = match (pair.load_pkgs)(pair.eco, pair.lock) {
        Ok(pkgs) => pkgs,
        Err(err) => {
            return Err(reported(stderr, err.into()));
        }
    };
    let cc_lock_sha = if pair.eco == Language::Cc {
        cc::parse_cc_lock_sha(pair.lock)
    } else {
        BTreeMap::new()
    };
    check_maps(pair.eco, &deps, &pkgs, &cc_lock_sha, stdout, stderr)
}

fn maven_deps(
    _eco: Language,
    artifacts: &Path,
) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    jvm::parse_maven_artifacts_list(artifacts)
}

fn maven_lock(_eco: Language, lock: &Path) -> Result<LockVersions, DepcheckError> {
    single(jvm::parse_jvm_lock(lock))
}

pub fn cmd_locks(
    locks: &WorkspaceLocks,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    let mut worst: Option<DxError> = None;
    for result in [
        check_pair(
            lock_pair(Language::Rust, locks.cargo_manifest, locks.cargo_lock),
            stdout,
            stderr,
        ),
        check_pair(
            lock_pair(Language::Python, locks.uv_manifest, locks.uv_lock),
            stdout,
            stderr,
        ),
        check_pair(
            lock_pair(Language::Js, locks.pnpm_manifest, locks.pnpm_lock),
            stdout,
            stderr,
        ),
        check_pair(
            lock_pair(Language::Go, locks.go_manifest, locks.go_lock),
            stdout,
            stderr,
        ),
        check_pair(
            Pair {
                eco: Language::Java,
                manifest: locks.maven_artifacts,
                lock: locks.maven_lock,
                load_deps: maven_deps,
                load_pkgs: maven_lock,
            },
            stdout,
            stderr,
        ),
        check_pair(
            lock_pair(Language::Csharp, locks.paket_manifest, locks.paket_lock),
            stdout,
            stderr,
        ),
        check_pair(
            lock_pair(Language::Ruby, locks.ruby_manifest, locks.ruby_lock),
            stdout,
            stderr,
        ),
    ] {
        if let Err(error) = result
            && worst
                .as_ref()
                .is_none_or(|w| error.exit_code() > w.exit_code())
        {
            worst = Some(error);
        }
    }
    worst.map_or(Ok(()), Err)
}
