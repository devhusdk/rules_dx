use super::*;

/// Refuses a package update in a pinned set; dx never rewrites its lock.
pub(super) fn selective(set: &ResolvedSet) -> BackendError {
    BackendError::Unsupported {
        set: set.name.clone(),
        reason: match set.ecosystem {
            Ecosystem::Ruby => {
                "dx does not resolve gems; edit Gemfile and regenerate Gemfile.lock with `bundle lock`"
            }
            Ecosystem::Conan => {
                "dx does not resolve Conan packages; edit conanfile and regenerate conan.lock with `conan lock create`"
            }
            Ecosystem::Fpm => {
                "dx does not resolve fpm packages; edit fpm.toml and regenerate fpm.lock by hand"
            }
            Ecosystem::Dub => {
                "dx does not resolve DUB packages; edit dub.json, run `dub upgrade`, and refresh dub.digests.json"
            }
            Ecosystem::Zig => {
                "dx does not resolve Zig packages; edit build.zig.zon and refresh zig.digests.json"
            }
            Ecosystem::Bicep => {
                "dx does not pick Bicep module versions; edit the ref in MODULE.bazel and set digest from `bazel run //bicep/private/tools:pin -- <ref>`"
            }
            Ecosystem::LuaRocks => {
                "dx checks no LuaRocks pins; run `dx update` to move them and rewrite luarocks.digests.json"
            }
            _ => {
                "dx does not resolve PowerShell modules; edit PSGallery.requirements.psd1 and regenerate PSGallery.lock.json by hand"
            }
        },
    }
}

pub(super) fn plan(set: &ResolvedSet, request: &Selection) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => Ok(BackendPlan::Noop),
        Selection::Packages(_) => Err(selective(set)),
    }
}

pub(super) fn check(set: &ResolvedSet, request: &Selection) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Pinned),
        Selection::Packages(_) => Err(selective(set)),
    }
}
