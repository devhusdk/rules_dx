use std::path::Path;

use super::AdoptError;
use crate::scaffold::Applied;

pub const LAUNCHER_SH_REL: &str = "tools/dx";

pub const LAUNCHER_CMD_REL: &str = "tools/dx.cmd";

/// Release assets the launchers can download, one sha256 pin each.
pub const RELEASE_ASSETS: [&str; 5] = [
    "dx-linux-x86_64",
    "dx-linux-arm64",
    "dx-macos-arm64",
    "dx-windows-x86_64",
    "dx-windows-arm64",
];

/// First word of the advisory snapshot lines `.dx/pins` also holds.
pub const ADVISORY_PIN_WORD: &str = "advisory";

/// The sha256 of each release asset, stored in `.dx/pins`.
/// An empty pin means "no verified hash": the launcher refuses to download.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LauncherPins {
    sha256: [String; 5],
}

impl LauncherPins {
    /// All-empty pins, for a version that has no release yet.
    pub fn unreleased() -> Self {
        Self::default()
    }

    /// The pin for `asset`, or `None` when `asset` is not a release asset.
    pub fn get(&self, asset: &str) -> Option<&str> {
        RELEASE_ASSETS
            .iter()
            .position(|known| *known == asset)
            .map(|i| self.sha256[i].as_str())
    }

    /// Parses a `dx-sha256sums.txt` manifest: exactly one `<hash>  <asset>`
    /// line per release asset, hash as 64 lowercase hex, nothing else.
    pub fn from_sha256sums(text: &str) -> Result<Self, AdoptError> {
        let mut pins = Self::default();
        let mut seen_archives = std::collections::BTreeSet::new();
        for line in text.lines() {
            let Some((hash, asset)) = line.split_once("  ") else {
                return Err(pins_error(format!("bad manifest line {line:?}")));
            };
            let archive = RELEASE_ASSETS
                .iter()
                .any(|known| asset == archive_name(known) || asset == symbols_name(known));
            let sbom = asset.strip_suffix(".spdx.json").is_some_and(|base| {
                RELEASE_ASSETS
                    .iter()
                    .any(|known| base == archive_name(known) || base == symbols_name(known))
            });
            if archive || sbom {
                if !seen_archives.insert(asset.to_owned()) {
                    return Err(pins_error(format!("{asset} is listed twice")));
                }
                continue;
            }
            let Some(i) = RELEASE_ASSETS.iter().position(|known| *known == asset) else {
                return Err(pins_error(format!("unknown asset {asset:?}")));
            };
            if hash.is_empty() || !valid_pin(hash) {
                return Err(pins_error(format!("{asset} hash is not 64 lowercase hex")));
            }
            if !pins.sha256[i].is_empty() {
                return Err(pins_error(format!("{asset} is listed twice")));
            }
            pins.sha256[i] = hash.to_owned();
        }
        if let Some(i) = pins.sha256.iter().position(String::is_empty) {
            return Err(pins_error(format!("{} is missing", RELEASE_ASSETS[i])));
        }
        Ok(pins)
    }

    /// Parses `.dx/pins`: `<version> <sha256> <asset>` lines for one version, any
    /// asset may be absent. A bare `<version>` line carries no hash.
    pub fn from_pin_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Result<Self, AdoptError> {
        let mut pins = Self::default();
        let mut version = None;
        for line in lines {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.first() == Some(&ADVISORY_PIN_WORD) {
                continue;
            }
            let (line_version, hash, asset) = match fields.as_slice() {
                [] => continue,
                [version] => (*version, None, None),
                [version, hash, asset] => (*version, Some(*hash), Some(*asset)),
                _ => return Err(pins_error(format!("bad pin line {line:?}"))),
            };
            if *version.get_or_insert(line_version) != line_version {
                return Err(pins_error("pins list more than one version".to_owned()));
            }
            let (Some(hash), Some(asset)) = (hash, asset) else {
                continue;
            };
            let Some(i) = RELEASE_ASSETS.iter().position(|known| *known == asset) else {
                return Err(pins_error(format!("unknown asset {asset:?}")));
            };
            if hash.is_empty() || !valid_pin(hash) {
                return Err(pins_error(format!("{asset} hash is not 64 lowercase hex")));
            }
            if !pins.sha256[i].is_empty() {
                return Err(pins_error(format!("{asset} is listed twice")));
            }
            pins.sha256[i] = hash.to_owned();
        }
        Ok(pins)
    }

    /// One `<version> <sha256> <asset>` line per non-empty pin, in `RELEASE_ASSETS`
    /// order; a bare `<version>` line when there is no pin.
    pub fn to_lines(&self, version: &str) -> String {
        let lines: String = RELEASE_ASSETS
            .iter()
            .zip(&self.sha256)
            .filter(|(_, pin)| !pin.is_empty())
            .map(|(asset, pin)| format!("{version} {pin} {asset}\n"))
            .collect();
        if lines.is_empty() {
            format!("{version}\n")
        } else {
            lines
        }
    }
}

/// The sha256 pins for dx `version`'s release assets.
///
/// Read from the `cli/prebuilt/pins.bzl` in the running binary's runfiles, which under
/// `bazel run` is the resolved module's copy, then from the embedded copy. A release binary is built from the
/// tag, where the embedded pins are empty, so it falls back to the `dx-sha256sums.txt`
/// that sits next to the running executable. Unreleased (empty) when neither has pins.
pub fn release_pins(version: &str) -> Result<LauncherPins, AdoptError> {
    if let Some(text) = runfiles_pins_bzl() {
        let pins = pins_from_bzl(&text, version)?;
        if pins != LauncherPins::unreleased() {
            return Ok(pins);
        }
    }
    let embedded = pins_from_bzl(include_str!("../../prebuilt/pins.bzl"), version)?;
    if embedded != LauncherPins::unreleased() {
        return Ok(embedded);
    }
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return Ok(embedded);
    };
    pins_from_manifest_dir(&dir)
}

fn runfiles_pins_bzl() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let runfiles = dx_path::Resolver::for_binary(&exe).ok()?;
    ["rules_dx", "_main"]
        .iter()
        .find_map(|repo| {
            runfiles
                .lookup(&format!("{repo}/cli/prebuilt/pins.bzl"))
                .ok()
        })
        .and_then(|path| std::fs::read_to_string(path).ok())
}

/// Pins from `dir/dx-sha256sums.txt`; unreleased when the file is absent.
pub fn pins_from_manifest_dir(dir: &Path) -> Result<LauncherPins, AdoptError> {
    match std::fs::read_to_string(dir.join("dx-sha256sums.txt")) {
        Ok(text) => LauncherPins::from_sha256sums(&text).map_err(|e| {
            pins_error(format!(
                "{}: {e}; writing .dx/pins without hashes",
                dir.join("dx-sha256sums.txt").display()
            ))
        }),
        Err(_) => Ok(LauncherPins::unreleased()),
    }
}

fn pins_from_bzl(text: &str, version: &str) -> Result<LauncherPins, AdoptError> {
    let mut pinned_version = None;
    let mut pins = LauncherPins::default();
    let mut in_sha256 = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DX_VERSION") {
            pinned_version = quoted(rest.trim_start().strip_prefix('=').unwrap_or(""));
        } else if line.starts_with("SHA256") {
            in_sha256 = true;
        } else if in_sha256 && line.starts_with('}') {
            in_sha256 = false;
        } else if in_sha256 {
            let Some((asset, hash)) = line.split_once(':') else {
                continue;
            };
            let (Some(asset), Some(hash)) = (quoted(asset), quoted(hash.trim_end_matches(',')))
            else {
                return Err(pins_error(format!("bad pins.bzl line {line:?}")));
            };
            let Some(i) = RELEASE_ASSETS.iter().position(|known| *known == asset) else {
                return Err(pins_error(format!("unknown asset {asset:?}")));
            };
            if !valid_pin(hash) {
                return Err(pins_error(format!("{asset} hash is not 64 lowercase hex")));
            }
            pins.sha256[i] = hash.to_owned();
        }
    }
    if pinned_version != Some(version) || pins.sha256.iter().any(String::is_empty) {
        return Ok(LauncherPins::unreleased());
    }
    Ok(pins)
}

fn quoted(text: &str) -> Option<&str> {
    text.trim().strip_prefix('"')?.strip_suffix('"')
}

/// The archive asset name for a raw release asset: `.zip` on Windows, `.tar.gz` elsewhere.
fn archive_name(raw: &str) -> String {
    let suffix = if raw.contains("windows") {
        ".zip"
    } else {
        ".tar.gz"
    };
    format!("{raw}{suffix}")
}

/// The debug symbols asset for a raw release asset: `<raw>-pdb.zip` on Windows, empty elsewhere.
fn symbols_name(raw: &str) -> String {
    if raw.contains("windows") {
        format!("{raw}-pdb.zip")
    } else {
        String::new()
    }
}

fn valid_pin(pin: &str) -> bool {
    pin.is_empty()
        || (pin.len() == 64 && pin.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

fn pins_error(detail: String) -> AdoptError {
    AdoptError::LauncherPins { detail }
}

const LAUNCHER_SH: &str = r##"#!/bin/sh
set -eu
DX_PINS=.dx/pins
fail() {
  echo "dx: $1" >&2
  echo "dx: run it through Bazel instead: bazel run @rules_dx//:dx -- <your arguments>" >&2
  exit 1
}
via_bazel() {
  echo "dx: $1, building from source through Bazel" >&2
  shift
  exec bazel run @rules_dx//:dx -- "$@"
}
root=$(CDPATH="" cd -- "$(dirname -- "$0")/.." && pwd)
os=
exe=
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  MINGW* | MSYS* | CYGWIN*)
    os=windows
    exe=.exe
    MSYS2_ARG_CONV_EXCL='*'
    export MSYS2_ARG_CONV_EXCL
    ;;
esac
# Hand off to the launcher of the checkout we are run from (a worktree slot or
# another checkout of the same git repo), so that checkout's pin applies.
# `worktree hook` never hands off: hooks belong to the dx that wrote them.
if [ "${1:-}" != worktree ] || [ "${2:-}" != hook ]; then
  here=$(pwd -P) || here=
  while [ -n "$here" ]; do
    if [ -x "$here/tools/dx" ] && [ -f "$here/.dx/pins" ]; then
      break
    fi
    if [ "$here" = / ]; then
      here=
    else
      here=$(dirname -- "$here")
    fi
  done
  me=$(CDPATH="" cd -P -- "$root" && pwd -P) || me=
  if [ -n "$here" ] && [ "$here" != "$me" ] && command -v git > /dev/null 2>&1; then
    gcd() {
      (
        CDPATH=
        unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_PREFIX
        cd -P -- "$1" 2> /dev/null || exit 1
        c=$(git rev-parse --git-common-dir 2> /dev/null) || exit 1
        cd -P -- "$c" 2> /dev/null && pwd -P
      )
    }
    a=$(gcd "$here") || a=
    b=$(gcd "$me") || b=
    if [ -n "$a" ] && [ "$a" = "$b" ]; then
      exec "$here/tools/dx" "$@"
    fi
  fi
fi
pins="$root/$DX_PINS"
module="$root/MODULE.bazel"
[ -f "$module" ] || via_bazel "no MODULE.bazel" "$@"
if grep -Eq 'module_name *= *"rules_dx"' "$module"; then
  via_bazel "MODULE.bazel is rules_dx itself" "$@"
fi
dep='^bazel_dep\(name = "rules_dx", version = "[^"]*"\)[[:space:]]*$'
[ "$(grep -Ec "$dep" "$module" || true)" = 1 ] || via_bazel "MODULE.bazel needs exactly one plain rules_dx bazel_dep" "$@"
VERSION=$(grep -E "$dep" "$module" | sed 's/^.*version = "\([^"]*\)".*$/\1/')
case "$VERSION" in
  "" | .* | *[!0-9A-Za-z.+-]*) via_bazel "cannot read the rules_dx version in MODULE.bazel" "$@" ;;
esac
[ -n "$os" ] || via_bazel "unknown OS $(uname -s)" "$@"
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) via_bazel "unknown machine $(uname -m)" "$@" ;;
esac
if [ "$os-$arch" = macos-x86_64 ]; then
  for key in hw.optional.arm64 sysctl.proc_translated; do
    if [ "$(PATH="$PATH:/usr/sbin:/sbin" sysctl -n "$key" 2> /dev/null)" = 1 ]; then
      arch=arm64
      break
    fi
  done
fi
case "$os-$arch" in
  linux-x86_64 | linux-arm64 | macos-arm64 | windows-x86_64 | windows-arm64) ;;
  *) via_bazel "no release binary for $os-$arch" "$@" ;;
esac
if [ -n "${DX_CACHE_DIR:-}" ]; then
  cache="$DX_CACHE_DIR"
elif [ -n "${XDG_CACHE_HOME:-}" ]; then
  cache="$XDG_CACHE_HOME/dx"
elif [ "$os" = windows ]; then
  [ -n "${USERPROFILE:-}" ] || fail "USERPROFILE is not set"
  cache="$USERPROFILE/.cache/dx"
else
  [ -n "${HOME:-}" ] || fail "HOME is not set"
  cache="$HOME/.cache/dx"
fi
if [ "$os" = windows ]; then
  cache=$(cygpath -u "$cache")
fi
dir="$cache/bin/$VERSION"
bin="$dir/dx-$os-$arch$exe"
noexec="cannot execute $bin; is $cache on a noexec mount? set DX_CACHE_DIR to an executable directory"
if [ -f "$bin" ]; then
  [ -x "$bin" ] || fail "$noexec"
  exec "$bin" "$@"
fi
if [ "$os" = linux ]; then
  libc=$(getconf GNU_LIBC_VERSION 2> /dev/null || true)
  case "$libc" in
    "glibc "*)
      glibc=${libc#glibc }
      case "$glibc" in
        [01].* | 2.[0-9] | 2.[0-9].* | 2.1[0-9] | 2.1[0-9].* | 2.2[0-7] | 2.2[0-7].*)
          fail "dx needs glibc 2.28 or newer on Linux; this host uses glibc $glibc"
          ;;
      esac
      ;;
    *)
      case "$(ldd --version 2>&1 || true)" in
        *musl*) fail "dx needs glibc 2.28 or newer on Linux; this host uses musl" ;;
      esac
      ;;
  esac
fi
[ -f "$pins" ] || via_bazel "no pinned binary for $os-$arch at $VERSION" "$@"
cr=$(printf '\r')
want=
while read -r ver sum asset || [ -n "$ver" ]; do
  asset=${asset%"$cr"}
  if [ "$ver" = "$VERSION" ] && [ "$asset" = "dx-$os-$arch" ]; then
    want=$sum
  fi
done < "$pins"
[ -n "$want" ] || via_bazel "no pinned binary for $os-$arch at $VERSION" "$@"
url="https://github.com/devhusdk/rules_dx/releases/download/v$VERSION/dx-$os-$arch"
mkdir -p "$dir"
tmp="$dir/.dx-$os-$arch.$$"
trap 'rm -f "$tmp"' EXIT
fetch() {
  if command -v curl > /dev/null 2>&1; then
    curl -fsSL --connect-timeout 20 --speed-limit 1024 --speed-time 30 --retry 3 -o "$2" "$1"
  elif command -v wget > /dev/null 2>&1; then
    wget -q --timeout=30 --tries=3 -O "$2" "$1"
  else
    return 1
  fi
}
fetch "$url" "$tmp" || {
  status=$?
  rm -f "$tmp"
  rmdir "$dir" 2> /dev/null || true
  via_bazel "download of $url failed (exit $status)" "$@"
}
if command -v sha256sum > /dev/null 2>&1; then
  got=$(sha256sum "$tmp" | cut -d' ' -f1)
elif command -v shasum > /dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
else
  fail "needs sha256sum or shasum"
fi
[ "$got" = "$want" ] || fail "sha256 mismatch for $url: want '$want', got $got; refusing to run it"
chmod +x "$tmp"
if ! mv "$tmp" "$bin" && [ -x "$bin" ]; then
  rm -f "$tmp"
fi
[ -x "$bin" ] || fail "$noexec"
exec "$bin" "$@"
"##;

const LAUNCHER_CMD: &str = r##"@echo off
setlocal
set "DX_PINS=.dx\pins"
set "ARCH=x86_64"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ARCH=arm64"
set "DX_HERE="
if /i "%~1"=="worktree" if /i "%~2"=="hook" goto dxdone
set "DX_HERE=%CD%"
:dxup
if exist "%DX_HERE%\tools\dx.cmd" if exist "%DX_HERE%\.dx\pins" goto dxfound
for %%I in ("%DX_HERE%\..") do set "DX_NEXT=%%~fI"
if /i "%DX_NEXT%"=="%DX_HERE%" (
  set "DX_HERE="
  goto dxdone
)
set "DX_HERE=%DX_NEXT%"
goto dxup
:dxfound
for %%I in ("%~dp0..") do set "DX_ME=%%~fI"
if /i "%DX_HERE%"=="%DX_ME%" goto dxdone
set "GIT_DIR="
set "GIT_WORK_TREE="
set "GIT_COMMON_DIR="
set "GIT_INDEX_FILE="
set "GIT_OBJECT_DIRECTORY="
set "GIT_PREFIX="
set "DX_GA="
set "DX_GB="
pushd "%DX_HERE%" 2>nul && (for /f "delims=" %%G in ('git rev-parse --git-common-dir 2^>nul') do if not defined DX_GA set "DX_GA=%%~fG") & popd
pushd "%DX_ME%" 2>nul && (for /f "delims=" %%G in ('git rev-parse --git-common-dir 2^>nul') do if not defined DX_GB set "DX_GB=%%~fG") & popd
if not defined DX_GA goto dxdone
if /i not "%DX_GA%"=="%DX_GB%" goto dxdone
"%DX_HERE%\tools\dx.cmd" %*
exit /b %errorlevel%
:dxdone
set "DX_MODULE=%~dp0..\MODULE.bazel"
if not exist "%DX_MODULE%" goto bazel
findstr /r /c:"module_name *= *.rules_dx." "%DX_MODULE%" >nul && goto bazel
set "DX_N=0"
set "VERSION="
for /f tokens^=4^ delims^=^" %%V in ('findstr /r /c:"^^bazel_dep(name = .rules_dx., version = .[0-9A-Za-z.+-]*.)[ ]*$" "%DX_MODULE%"') do (set /a DX_N+=1 >nul & set "VERSION=%%V")
if not "%DX_N%"=="1" goto bazel
set VERSION | findstr /r /x "VERSION=[0-9A-Za-z+-][0-9A-Za-z.+-]*" >nul
if errorlevel 1 goto bazel
set "DX_CACHE=%USERPROFILE%\.cache\dx"
if defined XDG_CACHE_HOME set "DX_CACHE=%XDG_CACHE_HOME%\dx"
if defined DX_CACHE_DIR set "DX_CACHE=%DX_CACHE_DIR%"
set "DIR=%DX_CACHE%\bin\%VERSION%"
set "BIN=%DIR%\dx-windows-%ARCH%.exe"
if exist "%BIN%" goto run
if not exist "%~dp0..\%DX_PINS%" goto bazel
set "WANT="
for /f "usebackq tokens=1,2,3" %%A in ("%~dp0..\%DX_PINS%") do if "%%A"=="%VERSION%" if "%%C"=="dx-windows-%ARCH%" set "WANT=%%B"
if not defined WANT goto bazel
set "DX_URL=https://github.com/devhusdk/rules_dx/releases/download/v%VERSION%/dx-windows-%ARCH%"
mkdir "%DIR%" 2>nul
set "DX_TAG="
for /f %%G in ('powershell -NoProfile -NonInteractive -Command "[guid]::NewGuid().ToString()"') do set "DX_TAG=%%G"
if not defined DX_TAG goto bazel
set "DX_PART=%DIR%\.dx-%ARCH%.%DX_TAG%.part"
call powershell -NoProfile -NonInteractive -Command "$ErrorActionPreference='Stop'; Invoke-WebRequest -UseBasicParsing -ProxyUseDefaultCredentials -Uri $env:DX_URL -OutFile $env:DX_PART"
if errorlevel 1 (
  call :clean
  rmdir "%DIR%" 2>nul
  goto bazel
)
set "GOT="
for /f "skip=1 tokens=*" %%H in ('certutil -hashfile "%DX_PART%" SHA256') do if not defined GOT set "GOT=%%H"
set "GOT=%GOT: =%"
if /i not "%GOT%"=="%WANT%" goto mismatch
move /y "%DX_PART%" "%BIN%" >nul 2>&1
call :clean
if exist "%BIN%" goto run
echo dx: could not install %BIN% 1>&2
echo dx: run it through Bazel instead: bazel run @rules_dx//:dx -- ^<your arguments^> 1>&2
exit /b 1
:run
"%BIN%" %*
exit /b %errorlevel%
:mismatch
call :clean
echo dx: sha256 mismatch for %DX_URL%: want '%WANT%', got '%GOT%'; refusing to run it 1>&2
echo dx: run it through Bazel instead: bazel run @rules_dx//:dx -- ^<your arguments^> 1>&2
exit /b 1
:clean
del /q "%DX_PART%" 2>nul
exit /b 0
:bazel
bazel run @rules_dx//:dx -- %*
exit /b %errorlevel%
"##;

pub fn render_launcher_sh() -> String {
    LAUNCHER_SH.to_owned()
}

pub fn render_launcher_cmd() -> String {
    LAUNCHER_CMD.replace('\n', "\r\n")
}

fn write_launcher(
    root: &Path,
    rel: &str,
    content: &str,
    executable: bool,
) -> Result<(), AdoptError> {
    let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
            parent: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    let write_error = |e: &dyn std::fmt::Display| AdoptError::WriteFile {
        path: dest.display().to_string(),
        detail: e.to_string(),
    };
    dx_atomic_fs::write_atomic(&dest, content.as_bytes()).map_err(|e| write_error(&e))?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| write_error(&e))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Writes absent or stale launchers, recording each in `applied` as it lands.
pub fn write_launcher_files(root: &Path, applied: &mut Applied) -> Result<(), AdoptError> {
    sync(root, true, true, applied)
}

pub fn refresh_launcher_files(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut applied = Applied::default();
    sync(root, false, true, &mut applied)?;
    Ok(applied.wrote)
}

pub fn plan_launcher_files(root: &Path, create_missing: bool) -> Result<Vec<String>, AdoptError> {
    let mut applied = Applied::default();
    sync(root, create_missing, false, &mut applied)?;
    Ok(applied.wrote)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherState {
    Current,
    Stale,
}

fn static_launcher(rel: &str) -> String {
    if rel == LAUNCHER_SH_REL {
        render_launcher_sh()
    } else {
        render_launcher_cmd()
    }
}

/// The command that runs `dx init --apply` in `root`: the launcher when it is there, else Bazel.
pub fn init_apply_command(root: &Path) -> String {
    if root.join(LAUNCHER_SH_REL).is_file() {
        format!("{LAUNCHER_SH_REL} init --apply")
    } else {
        "bazel run @rules_dx//:dx -- init --apply".to_owned()
    }
}

/// `None` when the file is absent or custom; dx-owned files are `Current` or `Stale`.
pub fn launcher_state(root: &Path, rel: &str) -> Option<LauncherState> {
    let dest = crate::scaffold::scaffold_dest_within_root(root, rel).ok()?;
    let text = std::fs::read_to_string(&dest).ok()?;
    let mut lines = text.lines().map(|line| line.trim_end_matches('\r'));
    let (first, second, marker) = match rel {
        LAUNCHER_SH_REL => ("#!/bin/sh", "set -eu", "DX_PINS=.dx/pins"),
        LAUNCHER_CMD_REL => ("@echo off", "setlocal", "set \"DX_PINS=.dx\\pins\""),
        _ => return None,
    };
    if lines.next()? != first || lines.next()? != second {
        return None;
    }
    if lines.next()? != marker {
        return None;
    }
    if text != static_launcher(rel) || (rel == LAUNCHER_SH_REL && !is_executable(&dest)) {
        return Some(LauncherState::Stale);
    }
    Some(LauncherState::Current)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 == 0o111)
}

#[cfg(not(unix))]
fn is_executable(_: &Path) -> bool {
    true
}

pub fn stale_launchers(root: &Path) -> Vec<String> {
    [LAUNCHER_SH_REL, LAUNCHER_CMD_REL]
        .into_iter()
        .filter(|rel| launcher_state(root, rel) == Some(LauncherState::Stale))
        .map(str::to_owned)
        .collect()
}

fn sync(
    root: &Path,
    create_missing: bool,
    apply: bool,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    for rel in [LAUNCHER_SH_REL, LAUNCHER_CMD_REL] {
        let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
        let write = if dest.exists() {
            launcher_state(root, rel) == Some(LauncherState::Stale)
        } else {
            create_missing
        };
        if !write {
            applied.refused.push(rel.to_owned());
            continue;
        }
        if apply {
            write_launcher(root, rel, &static_launcher(rel), rel == LAUNCHER_SH_REL)?;
        }
        applied.wrote.push(rel.to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_version_pins;

    fn pinned(sha: &str) -> LauncherPins {
        LauncherPins {
            sha256: std::array::from_fn(|_| sha.to_owned()),
        }
    }

    fn sh() -> String {
        render_launcher_sh()
    }

    #[test]
    fn rendered_sh_launcher_passes_shellcheck() {
        let shellcheck = dx_testing::resolve_runfiles(
            &std::env::var("DX_SHELLCHECK").expect("DX_SHELLCHECK names the pinned shellcheck"),
        );
        let dir = dx_test_scratch::scratch("launcher-shellcheck");
        let script = dir.path().join("dx");
        std::fs::write(&script, sh()).expect("write launcher");
        let run = std::process::Command::new(shellcheck)
            .args(["--norc", "--shell=sh", "--severity=style"])
            .arg(&script)
            .output()
            .expect("shellcheck runs");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stdout)
        );
    }

    fn cmd() -> String {
        render_launcher_cmd()
    }

    const MANIFEST: &str =
        "1111111111111111111111111111111111111111111111111111111111111111  dx-linux-x86_64
2222222222222222222222222222222222222222222222222222222222222222  dx-linux-arm64
3333333333333333333333333333333333333333333333333333333333333333  dx-macos-arm64
4444444444444444444444444444444444444444444444444444444444444444  dx-windows-x86_64
5555555555555555555555555555555555555555555555555555555555555555  dx-windows-arm64
";

    #[test]
    fn consumer_launchers_carry_no_self_build() {
        let sh = sh();
        let cmd = cmd();
        for script in [&sh, &cmd] {
            for gone in [
                "bazel-bin",
                ".dx/local/launcher",
                ".dx\\local\\launcher",
                "//:dx\"",
                "build //:dx",
                "--nohome_rc",
                "disk_cache",
                "cli/cli/BUILD.bazel",
                "cli\\cli\\BUILD.bazel",
            ] {
                assert!(!script.contains(gone), "{gone}");
            }
        }
        assert_eq!(cmd.matches("\r\n:run\r\n").count(), 1);
        assert!(!cmd.contains(":pinned"));
        let os = sh.find("\nos=\n").expect("os");
        let msys = sh
            .find("\n    export MSYS2_ARG_CONV_EXCL\n")
            .expect("msys export");
        let pin = sh.find("\nVERSION=$(grep").expect("pin check");
        let handoff = sh.find("exec \"$here/tools/dx\"").expect("handoff");
        assert!(os < msys && msys < handoff && handoff < pin);
    }
    #[test]
    fn launchers_are_static_and_read_the_pins_file() {
        let sh = sh();
        assert!(sh.starts_with("#!/bin/sh\n"));
        assert_eq!(sh.lines().nth(2), Some("DX_PINS=.dx/pins"));
        let cmd = cmd();
        assert_eq!(cmd.lines().nth(2), Some("set \"DX_PINS=.dx\\pins\""));
        assert!(!cmd.replace("\r\n", "").contains('\n'));
        for script in [&sh, &cmd] {
            for gone in ["@VERSION@", "@SHA256_", "VERSION=\"1", "SHA256_"] {
                assert!(!script.contains(gone), "{gone}");
            }
        }
    }

    #[test]
    fn cmd_hand_off_does_not_expand_arguments_twice() {
        let script = cmd();
        let line = script
            .lines()
            .find(|line| line.ends_with("dx.cmd\" %*"))
            .expect("hand-off line");
        assert_eq!(line, "\"%DX_HERE%\\tools\\dx.cmd\" %*");
    }

    #[test]
    fn cmd_download_answers_proxy_auth_with_default_credentials() {
        let script = cmd();
        let line = script
            .lines()
            .find(|line| line.contains("Invoke-WebRequest"))
            .expect("download line");
        assert!(line.contains("-ProxyUseDefaultCredentials"), "{line}");
    }

    #[test]
    fn launcher_bytes_do_not_depend_on_the_dx_version() {
        let dir = dx_test_scratch::scratch("dx-launcher-static-");
        let mut seen = Vec::new();
        for version in ["1.2.3", "2.0.0-rc.1"] {
            write_version_pins(dir.path(), version, &pinned(&"a".repeat(64))).expect("pin");
            write_launcher_files(dir.path(), &mut Applied::default()).expect("launchers");
            seen.push((
                std::fs::read(dir.path().join(LAUNCHER_SH_REL)).expect("sh"),
                std::fs::read(dir.path().join(LAUNCHER_CMD_REL)).expect("cmd"),
            ));
        }
        assert_eq!(seen[0], seen[1]);
        assert_eq!(seen[0].0, sh().into_bytes());
        assert_eq!(seen[0].1, cmd().into_bytes());
    }

    #[test]
    fn launcher_state_reads_line_three() {
        let dir = dx_test_scratch::scratch("dx-launcher-line3-");
        write_launcher(dir.path(), LAUNCHER_SH_REL, &sh(), true).expect("sh");
        write_launcher(dir.path(), LAUNCHER_CMD_REL, &cmd(), false).expect("cmd");
        for rel in [LAUNCHER_SH_REL, LAUNCHER_CMD_REL] {
            assert_eq!(
                launcher_state(dir.path(), rel),
                Some(LauncherState::Current)
            );
        }
    }

    #[test]
    fn manifest_dir_gives_pins_and_missing_manifest_gives_none() {
        let dir = dx_test_scratch::scratch("dx-pins-manifest-");
        assert_eq!(
            pins_from_manifest_dir(dir.path()),
            Ok(LauncherPins::unreleased())
        );
        std::fs::write(dir.path().join("dx-sha256sums.txt"), "junk\n").expect("write");
        let err = pins_from_manifest_dir(dir.path()).expect_err("bad manifest");
        assert!(err.to_string().contains("without hashes"), "{err}");
        std::fs::write(dir.path().join("dx-sha256sums.txt"), MANIFEST).expect("write");
        let pins = pins_from_manifest_dir(dir.path()).expect("pins");
        assert_eq!(
            pins,
            LauncherPins::from_sha256sums(MANIFEST).expect("manifest")
        );
    }

    #[test]
    fn pin_lines_round_trip_and_allow_missing_assets() {
        let all = LauncherPins::from_sha256sums(MANIFEST).expect("manifest");
        let text = all.to_lines("1.2.3");
        assert_eq!(
            text.lines().next(),
            Some(format!("1.2.3 {} dx-linux-x86_64", "1".repeat(64)).as_str())
        );
        assert_eq!(LauncherPins::from_pin_lines(text.lines()), Ok(all));
        let some = LauncherPins::from_pin_lines(text.lines().take(2)).expect("some");
        assert_eq!(some.get("dx-linux-arm64"), Some("2".repeat(64).as_str()));
        assert_eq!(some.get("dx-macos-arm64"), Some(""));
        assert_eq!(
            LauncherPins::from_pin_lines(std::iter::empty()),
            Ok(LauncherPins::unreleased())
        );
        assert_eq!(LauncherPins::unreleased().to_lines("1.2.3"), "1.2.3\n");
        assert_eq!(
            LauncherPins::from_pin_lines("1.2.3\n".lines()),
            Ok(LauncherPins::unreleased())
        );
        let crlf = format!("{}\r", text.lines().next().expect("line"));
        assert_eq!(
            LauncherPins::from_pin_lines(std::iter::once(crlf.as_str()))
                .expect("crlf")
                .get("dx-linux-x86_64"),
            Some("1".repeat(64).as_str())
        );
    }

    #[test]
    fn pin_lines_reject_bad_lines() {
        let a = "a".repeat(64);
        for bad in [
            format!("1.2.3 {a}"),
            format!("1.2.3 {a} dx-macos-x86_64"),
            format!("1.2.3 {} dx-linux-x86_64", "A".repeat(64)),
            format!("1.2.3 {} dx-linux-x86_64", "a".repeat(63)),
            format!("1.2.3 {a} dx-linux-x86_64 extra"),
        ] {
            assert!(
                matches!(
                    LauncherPins::from_pin_lines(std::iter::once(bad.as_str())),
                    Err(AdoptError::LauncherPins { .. })
                ),
                "{bad:?}"
            );
        }
        let twice = format!("1.2.3 {a} dx-linux-x86_64\n1.2.3 {a} dx-linux-x86_64");
        assert!(LauncherPins::from_pin_lines(twice.lines()).is_err());
        let mixed = format!("1.2.3 {a} dx-linux-x86_64\n1.2.4 {a} dx-linux-arm64");
        assert!(LauncherPins::from_pin_lines(mixed.lines()).is_err());
    }

    #[test]
    fn from_sha256sums_skips_archive_lines() {
        let a = "a".repeat(64);
        let archives = format!(
            "{a}  dx-linux-x86_64.tar.gz\n{a}  dx-linux-arm64.tar.gz\n{a}  dx-macos-arm64.tar.gz\n\
             {a}  dx-windows-x86_64.zip\n{a}  dx-windows-arm64.zip\n"
        );
        let combined = format!("{MANIFEST}{archives}");
        assert_eq!(
            LauncherPins::from_sha256sums(&combined).expect("combined"),
            LauncherPins::from_sha256sums(MANIFEST).expect("manifest")
        );
        for extra in [
            format!("{a}  dx-linux-x86_64.tar.gz\n"),
            format!("{a}  dx-foo.tar.gz\n"),
            format!("{a}  dx-linux-x86_64.zip\n"),
            format!("{a}  dx-windows-arm64.tar.gz\n"),
        ] {
            let text = format!("{combined}{extra}");
            assert!(LauncherPins::from_sha256sums(&text).is_err(), "{extra:?}");
        }
        let unknown = format!("{MANIFEST}{a}  dx-foo.tar.gz\n");
        assert!(LauncherPins::from_sha256sums(&unknown).is_err());
    }

    #[test]
    fn from_sha256sums_skips_symbols_lines() {
        let a = "a".repeat(64);
        let symbols =
            format!("{a}  dx-windows-x86_64-pdb.zip\n{a}  dx-windows-arm64-pdb.zip.spdx.json\n");
        assert_eq!(
            LauncherPins::from_sha256sums(&format!("{MANIFEST}{symbols}")).expect("combined"),
            LauncherPins::from_sha256sums(MANIFEST).expect("manifest")
        );
        let linux = format!("{MANIFEST}{a}  dx-linux-x86_64-pdb.zip\n");
        assert!(LauncherPins::from_sha256sums(&linux).is_err());
    }

    #[test]
    fn from_sha256sums_skips_sbom_lines() {
        let a = "a".repeat(64);
        let sbom = format!("{a}  dx-linux-x86_64.tar.gz.spdx.json\n");
        let combined = format!("{MANIFEST}{sbom}");
        assert_eq!(
            LauncherPins::from_sha256sums(&combined).expect("combined"),
            LauncherPins::from_sha256sums(MANIFEST).expect("manifest")
        );
        assert!(LauncherPins::from_sha256sums(&format!("{combined}{sbom}")).is_err());
        let unknown = format!("{MANIFEST}{a}  dx-foo.tar.gz.spdx.json\n");
        assert!(LauncherPins::from_sha256sums(&unknown).is_err());
    }

    #[test]
    fn from_sha256sums_rejects_missing_duplicate_extra_uppercase() {
        let pins = LauncherPins::from_sha256sums(MANIFEST).expect("manifest");
        assert_eq!(pins.get("dx-macos-arm64"), Some("3".repeat(64).as_str()));
        assert_eq!(pins.get("dx-macos-x86_64"), None);
        let lines: Vec<&str> = MANIFEST.lines().collect();
        let a = "a".repeat(64);
        let bad = [
            lines[..4].join("\n"),
            format!("{MANIFEST}{}\n", lines[0]),
            format!("{MANIFEST}{a}  dx-macos-x86_64\n"),
            MANIFEST.replace(&"1".repeat(64), &"A".repeat(64)),
            MANIFEST.replace(&"1".repeat(64), &"1".repeat(63)),
            MANIFEST.replace(&"1".repeat(64), ""),
            MANIFEST.replacen("  ", " ", 1),
            format!("{MANIFEST}\n"),
            format!("\n{MANIFEST}"),
            String::new(),
        ];
        for text in bad {
            assert!(
                matches!(
                    LauncherPins::from_sha256sums(&text),
                    Err(AdoptError::LauncherPins { .. })
                ),
                "{text:?}"
            );
        }
    }

    fn bzl(version: &str, hash: &str) -> String {
        let rows: String = RELEASE_ASSETS
            .iter()
            .map(|asset| format!("    \"{asset}\": \"{hash}\",\n"))
            .collect();
        format!("DX_VERSION = \"{version}\"\n\nSHA256 = {{\n{rows}}}\n")
    }

    #[test]
    fn pins_from_bzl_returns_pins_for_the_pinned_version() {
        let hash = "a".repeat(64);
        let pins = pins_from_bzl(&bzl("1.2.3", &hash), "1.2.3").expect("pins");
        for asset in RELEASE_ASSETS {
            assert_eq!(pins.get(asset), Some(hash.as_str()));
        }
    }

    #[test]
    fn pins_from_bzl_is_unreleased_for_other_versions_and_empty_values() {
        let hash = "a".repeat(64);
        assert_eq!(
            pins_from_bzl(&bzl("1.2.3", &hash), "1.2.4").expect("pins"),
            LauncherPins::unreleased()
        );
        assert_eq!(
            pins_from_bzl(&bzl("1.2.3", ""), "1.2.3").expect("pins"),
            LauncherPins::unreleased()
        );
    }

    #[test]
    fn pins_from_bzl_rejects_malformed_values() {
        assert!(matches!(
            pins_from_bzl(&bzl("1.2.3", "XYZ"), "1.2.3"),
            Err(AdoptError::LauncherPins { .. })
        ));
    }

    #[test]
    fn release_pins_of_unreleased_is_empty() {
        let pins = release_pins(crate::DX_VERSION).expect("pins");
        assert_eq!(pins, LauncherPins::unreleased());
        for asset in RELEASE_ASSETS {
            assert_eq!(pins.get(asset), Some(""));
        }
    }

    #[test]
    fn launchers_share_the_shim_cache_root() {
        let sh = sh();
        let dx = sh.find("cache=\"$DX_CACHE_DIR\"").expect("DX_CACHE_DIR");
        let xdg = sh
            .find("cache=\"$XDG_CACHE_HOME/dx\"")
            .expect("XDG_CACHE_HOME");
        let home = sh.find("cache=\"$HOME/.cache/dx\"").expect("HOME");
        assert!(dx < xdg && xdg < home);
        assert!(sh.contains("dir=\"$cache/bin/$VERSION\""));
        let cmd = cmd();
        let home = cmd
            .find("set \"DX_CACHE=%USERPROFILE%\\.cache\\dx\"\r\n")
            .expect("USERPROFILE");
        let xdg = cmd
            .find("if defined XDG_CACHE_HOME set \"DX_CACHE=%XDG_CACHE_HOME%\\dx\"\r\n")
            .expect("XDG_CACHE_HOME");
        let dx = cmd
            .find("if defined DX_CACHE_DIR set \"DX_CACHE=%DX_CACHE_DIR%\"\r\n")
            .expect("DX_CACHE_DIR");
        assert!(home < xdg && xdg < dx);
        assert!(cmd.contains("set \"DIR=%DX_CACHE%\\bin\\%VERSION%\"\r\n"));
    }

    #[test]
    fn write_replaces_absent_or_stale_launchers_only() {
        let dir = dx_test_scratch::scratch("dx-launcher-write-");
        let first =
            crate::scaffold::entries(|a| write_launcher_files(dir.path(), a)).expect("writes");
        assert_eq!(first, vec!["tools/dx", "tools/dx.cmd"]);
        let current =
            crate::scaffold::entries(|a| write_launcher_files(dir.path(), a)).expect("current");
        assert_eq!(current, vec!["refused:tools/dx", "refused:tools/dx.cmd"]);
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), "custom\n").expect("edit");
        std::fs::write(
            dir.path().join(LAUNCHER_CMD_REL),
            format!("{}rem edited\r\n", cmd()),
        )
        .expect("edit");
        let second =
            crate::scaffold::entries(|a| write_launcher_files(dir.path(), a)).expect("syncs");
        assert_eq!(second, vec!["tools/dx.cmd", "refused:tools/dx"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(LAUNCHER_SH_REL)).expect("read"),
            "custom\n"
        );
        assert_eq!(
            launcher_state(dir.path(), LAUNCHER_CMD_REL),
            Some(LauncherState::Current)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(LAUNCHER_CMD_REL))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0);
        }
    }

    #[test]
    fn an_edited_launcher_is_stale_then_rewritten() {
        let dir = dx_test_scratch::scratch("dx-launcher-refresh-");
        assert!(refresh_launcher_files(dir.path()).expect("none").is_empty());
        assert!(!dir.path().join(LAUNCHER_SH_REL).exists());
        std::fs::create_dir_all(dir.path().join("tools")).expect("tools");
        std::fs::write(
            dir.path().join(LAUNCHER_SH_REL),
            format!("{}# edited\n", render_launcher_sh()),
        )
        .expect("seed");
        assert_eq!(
            launcher_state(dir.path(), LAUNCHER_SH_REL),
            Some(LauncherState::Stale)
        );
        assert_eq!(stale_launchers(dir.path()), vec!["tools/dx"]);
        let wrote = refresh_launcher_files(dir.path()).expect("refresh");
        assert_eq!(wrote, vec!["tools/dx"]);
        assert!(!dir.path().join(LAUNCHER_CMD_REL).exists());
        assert!(stale_launchers(dir.path()).is_empty());
        assert!(
            refresh_launcher_files(dir.path())
                .expect("current")
                .is_empty()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(LAUNCHER_SH_REL))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_launcher_without_exec_bits_is_stale_and_restored() {
        use std::os::unix::fs::PermissionsExt;
        let dir = dx_test_scratch::scratch("dx-launcher-mode-");
        write_launcher(dir.path(), LAUNCHER_SH_REL, &sh(), true).expect("sh");
        write_launcher(dir.path(), LAUNCHER_CMD_REL, &cmd(), false).expect("cmd");
        let sh_path = dir.path().join(LAUNCHER_SH_REL);
        std::fs::set_permissions(&sh_path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(stale_launchers(dir.path()), vec!["tools/dx"]);
        let wrote = refresh_launcher_files(dir.path()).expect("refresh");
        assert_eq!(wrote, vec!["tools/dx"]);
        let mode = std::fs::metadata(&sh_path)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        assert!(stale_launchers(dir.path()).is_empty());
    }

    #[test]
    fn custom_launchers_are_not_dx_owned() {
        let dir = dx_test_scratch::scratch("dx-launcher-custom-");
        std::fs::create_dir_all(dir.path().join("tools")).expect("tools");
        std::fs::write(
            dir.path().join(LAUNCHER_SH_REL),
            "#!/bin/sh\nset -eu\nroot=$(pwd)\nexec bazel run //:dx -- \"$@\"\n",
        )
        .expect("self-build");
        std::fs::write(
            dir.path().join(LAUNCHER_CMD_REL),
            "@echo off\r\nsetlocal\r\nbazel run //:dx -- %*\r\n",
        )
        .expect("self-build cmd");
        assert_eq!(launcher_state(dir.path(), LAUNCHER_SH_REL), None);
        assert_eq!(launcher_state(dir.path(), LAUNCHER_CMD_REL), None);
        assert!(stale_launchers(dir.path()).is_empty());
        assert_eq!(launcher_state(dir.path(), "tools/missing"), None);
        std::fs::write(
            dir.path().join(LAUNCHER_SH_REL),
            "#!/bin/sh\nVERSION=\"$(cat .dx/pins)\"\nexec dx \"$@\"\n",
        )
        .expect("custom with VERSION");
        assert_eq!(launcher_state(dir.path(), LAUNCHER_SH_REL), None);
        assert!(
            refresh_launcher_files(dir.path())
                .expect("custom kept")
                .is_empty()
        );
    }

    #[test]
    fn plan_reports_writes_without_writing() {
        let dir = dx_test_scratch::scratch("dx-launcher-plan-");
        assert_eq!(
            plan_launcher_files(dir.path(), true).expect("plan"),
            vec!["tools/dx", "tools/dx.cmd"]
        );
        assert!(
            plan_launcher_files(dir.path(), false)
                .expect("plan")
                .is_empty()
        );
        assert!(!dir.path().join("tools").exists());
    }

    #[cfg(unix)]
    mod run {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        const PAYLOAD: &str = "#!/bin/sh\necho ran \"$@\"\n";

        fn spawn(cmd: &mut Command) -> std::process::Child {
            cmd.stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            let mut tries = 0;
            loop {
                match cmd.spawn() {
                    Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 50 => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    other => return other.expect("launcher starts"),
                }
            }
        }

        fn sha_of(path: &Path) -> String {
            let out = Command::new("sh")
                .arg("-c")
                .arg("sha256sum \"$0\" 2>/dev/null || shasum -a 256 \"$0\"")
                .arg(path)
                .output()
                .expect("hashes");
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .next()
                .expect("digest")
                .to_owned()
        }

        const UNAME_LINUX: &str =
            "#!/bin/sh\ncase \"$1\" in\n  -s) echo Linux ;;\n  -m) echo x86_64 ;;\nesac\n";

        struct Fixture {
            _guard: Box<dyn std::any::Any>,
            base: std::path::PathBuf,
        }

        impl Fixture {
            fn new(name: &str, curl_ok: bool, pinned_sha: Option<&str>) -> Self {
                let dir = dx_test_scratch::scratch(name);
                let root = dir.path().join("repo");
                std::fs::create_dir_all(root.join(".dx")).expect("dx dir");
                std::fs::create_dir_all(dir.path().join("stub")).expect("stub dir");
                std::fs::create_dir_all(dir.path().join("home")).expect("home");
                let tools = dir.path().join("tools");
                let missing = dx_test_scratch::link_host_tools(
                    &tools,
                    &[
                        "dirname", "tr", "mkdir", "cut", "chmod", "mv", "rm", "rmdir", "cp",
                        "grep", "sed",
                    ],
                )
                .expect("link tools");
                assert!(missing.is_empty(), "host lacks tools: {missing:?}");
                let hashers = dx_test_scratch::link_host_tools(&tools, &["sha256sum", "shasum"])
                    .expect("link hashers");
                assert!(hashers.len() < 2, "host has no sha256sum or shasum");
                let payload = dir.path().join("payload");
                std::fs::write(&payload, PAYLOAD).expect("payload");
                let sha = sha_of(&payload);
                std::fs::write(dir.path().join("sidecar"), format!("{sha}  dx\n")).expect("sha");
                let pin = pinned_sha.map_or(sha, str::to_owned);
                let pins = if pin.is_empty() {
                    LauncherPins::unreleased()
                } else {
                    pinned(&pin)
                };
                write_version_pins(&root, "1.2.3", &pins).expect("pins");
                std::fs::write(
                    root.join("MODULE.bazel"),
                    "module(name = \"consumer\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n",
                )
                .expect("module");
                let curl = dir.path().join("stub/curl");
                let body = if curl_ok {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; fi\n  url=\"$1\"\n  shift\ndone\ncase \"$url\" in\n  *.sha256) cp \"$STUB_SIDECAR\" \"$out\" ;;\n  *) cp \"$STUB_PAYLOAD\" \"$out\" ;;\nesac\n"
                } else {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nexit 6\n"
                };
                dx_testing::executable_script(&curl, body);
                write_launcher_files(&root, &mut Applied::default()).expect("launcher");
                let fx = Self {
                    base: dir.path().to_path_buf(),
                    _guard: Box::new(dir),
                };
                fx.stub("uname", UNAME_LINUX);
                fx.stub("bazel", "#!/bin/sh\necho \"$@\" >> \"$STUB_LOG.bazel\"\n");
                fx
            }

            fn set_module(&self, text: &str) {
                std::fs::write(self.base.join("repo/MODULE.bazel"), text).expect("module");
            }

            fn bazel_calls(&self) -> Vec<String> {
                std::fs::read_to_string(self.base.join("calls.bazel"))
                    .map(|text| text.lines().map(str::to_owned).collect())
                    .unwrap_or_default()
            }

            fn assert_via_bazel(&self, args: &[&str]) -> String {
                let (code, out, err) = self.run(args);
                assert_eq!(code, Some(0), "{out}{err}");
                assert!(out.is_empty(), "{out}{err}");
                assert!(
                    err.starts_with("dx: ")
                        && err.ends_with(", building from source through Bazel\n"),
                    "{err}"
                );
                let want = format!("run @rules_dx//:dx -- {}", args.join(" "));
                assert_eq!(self.bazel_calls(), vec![want]);
                assert!(self.cached().is_empty(), "{:?}", self.cached());
                err
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
                self.run_in(&self.base, args)
            }

            fn run_in(
                &self,
                cwd: &std::path::Path,
                args: &[&str],
            ) -> (Option<i32>, String, String) {
                let base = &self.base;
                let out = spawn(
                    Command::new(base.join("repo/tools/dx"))
                        .current_dir(cwd)
                        .args(args)
                        .env_clear()
                        .env(
                            "PATH",
                            format!(
                                "{}:{}",
                                base.join("stub").display(),
                                base.join("tools").display()
                            ),
                        )
                        .env("HOME", base.join("home"))
                        .env("USERPROFILE", base.join("home"))
                        .env("STUB_LOG", base.join("calls"))
                        .env("STUB_PAYLOAD", base.join("payload"))
                        .env("STUB_SIDECAR", base.join("sidecar")),
                )
                .wait_with_output()
                .expect("launcher runs");
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                    String::from_utf8_lossy(&out.stderr).into_owned(),
                )
            }

            fn stub(&self, name: &str, body: &str) {
                let path = self.base.join("stub").join(name);
                dx_testing::executable_script(&path, body);
            }

            fn set_pin(&self, text: &str) {
                std::fs::write(self.base.join("repo/.dx/pins"), text).expect("pin");
            }

            fn rewrite_launcher(&self, from: &str, to: &str) {
                let path = self.base.join("repo/tools/dx");
                let text = std::fs::read_to_string(&path).expect("read launcher");
                assert!(text.contains(from), "launcher template lost {from:?}");
                dx_testing::executable_script(&path, &text.replacen(from, to, 1));
            }

            fn downloads(&self) -> usize {
                std::fs::read_to_string(self.base.join("calls"))
                    .map_or(0, |text| text.lines().count())
            }

            fn cached(&self) -> Vec<String> {
                std::fs::read_dir(self.base.join("home/.cache/dx/bin/1.2.3"))
                    .map(|entries| {
                        entries
                            .flatten()
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            }
        }

        impl Fixture {
            /// A second checkout of the same repo whose launcher prints `slot ARGS`.
            fn add_slot(&self, with_git: bool) -> std::path::PathBuf {
                let slot = self.base.join("slot");
                std::fs::create_dir_all(slot.join(".dx")).expect("slot dx");
                std::fs::create_dir_all(slot.join("tools")).expect("slot tools");
                std::fs::create_dir_all(slot.join("sub")).expect("slot sub");
                std::fs::write(slot.join(".dx/pins"), "9.9.9\n").expect("slot pin");
                let launcher = slot.join("tools/dx");
                dx_testing::executable_script(&launcher, "#!/bin/sh\necho slot \"$@\"\n");
                if with_git {
                    let common = format!("{}/repo/.git", self.base.display());
                    self.stub(
                        "git",
                        &format!(
                            "#!/bin/sh\necho git >> \"$STUB_LOG.git\"\ncase \"$PWD\" in\n  */repo) echo .git ;;\n  */slot*) echo {common} ;;\n  *) echo \"$PWD/.other\" ;;\nesac\n"
                        ),
                    );
                    std::fs::create_dir_all(self.base.join("repo/.git")).expect("git dir");
                }
                slot
            }

            fn git_calls(&self) -> usize {
                std::fs::read_to_string(self.base.join("calls.git"))
                    .map_or(0, |text| text.lines().count())
            }
        }

        #[test]
        fn hands_off_to_the_slot_launcher_before_the_pin_check() {
            let fx = Fixture::new("dx-launcher-handoff-", true, None);
            let slot = fx.add_slot(true);
            std::fs::write(fx.base.join("repo/.dx/pins"), "0.0.1\n").expect("stale pin");
            let (code, out, err) = fx.run_in(&slot.join("sub"), &["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "slot status\n"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn does_not_hand_off_to_another_repo_or_without_git() {
            let fx = Fixture::new("dx-launcher-nohandoff-", true, None);
            let slot = fx.add_slot(true);
            fx.stub(
                "git",
                "#!/bin/sh\ncase \"$PWD\" in\n  */repo) echo .git ;;\n  *) echo \"$PWD/.git\" ;;\nesac\n",
            );
            let (code, out, err) = fx.run_in(&slot, &["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            std::fs::remove_file(fx.base.join("stub/git")).expect("rm git");
            let (code, out, err) = fx.run_in(&slot, &["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        }

        #[test]
        fn hooks_and_own_checkout_never_start_git() {
            let fx = Fixture::new("dx-launcher-nogit-", true, None);
            let slot = fx.add_slot(true);
            let (code, out, err) = fx.run_in(&slot, &["worktree", "hook", "guard"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran worktree hook guard\n"),
                "{err}"
            );
            let (code, out, err) = fx.run_in(&fx.base.join("repo"), &["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.git_calls(), 0);
        }

        #[test]
        fn a_lost_mv_race_runs_the_installed_binary() {
            let fx = Fixture::new("sh-mv-race", true, None);
            fx.stub(
                "mv",
                "#!/bin/sh\nprintf '#!/bin/sh\\necho raced \"$@\"\\nexit 7\\n' > \"$2\"\nchmod +x \"$2\"\nexit 1\n",
            );
            let (code, out, _) = fx.run(&["a"]);
            assert_eq!((code, out.trim()), (Some(7), "raced a"));
        }

        #[test]
        fn a_failed_mv_without_an_installed_binary_fails() {
            let fx = Fixture::new("sh-mv-fail", true, None);
            fx.stub("mv", "#!/bin/sh\nexit 1\n");
            let (code, _, _) = fx.run(&["a"]);
            assert_ne!(code, Some(0));
        }

        #[test]
        fn musl_and_old_glibc_hosts_fail_before_downloading() {
            let fx = Fixture::new("sh-musl", true, None);
            fx.stub("getconf", "#!/bin/sh\nexit 1\n");
            fx.stub("ldd", "#!/bin/sh\necho 'musl libc (x86_64)'\n");
            let (code, _, err) = fx.run(&["status"]);
            assert_eq!(code, Some(1), "{err}");
            assert!(
                err.contains("glibc 2.28 or newer") && err.contains("musl"),
                "{err}"
            );
            assert_eq!(fx.downloads(), 0);
            fx.stub("getconf", "#!/bin/sh\necho 'glibc 2.17'\n");
            let (code, _, err) = fx.run(&["status"]);
            assert_eq!(code, Some(1), "{err}");
            assert!(err.contains("glibc 2.28 or newer"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn new_glibc_and_unknown_hosts_proceed() {
            let fx = Fixture::new("sh-glibc-ok", true, None);
            fx.stub("getconf", "#!/bin/sh\necho 'glibc 2.31'\n");
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            let fx = Fixture::new("sh-glibc-unknown", true, None);
            fx.stub("getconf", "#!/bin/sh\nexit 1\n");
            let (code, _, err) = fx.run(&["status"]);
            assert_eq!(code, Some(0), "{err}");
        }

        #[test]
        fn downloads_once_then_only_execs() {
            let fx = Fixture::new("dx-launcher-run-", true, None);
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 1, "the sidecar is never fetched");
            assert_eq!(fx.cached(), vec!["dx-linux-x86_64"]);
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 1, "cached run must not download");
        }

        #[test]
        fn a_cached_binary_that_cannot_run_fails_without_a_download() {
            let fx = Fixture::new("dx-launcher-noexec-", false, None);
            let dir = fx.base.join("home/.cache/dx/bin/1.2.3");
            std::fs::create_dir_all(&dir).expect("cache dir");
            let bin = dir.join("dx-linux-x86_64");
            std::fs::write(&bin, PAYLOAD).expect("cached");
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644)).expect("chmod");
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(out.is_empty(), "{out}");
            assert!(err.contains("noexec mount"), "{err}");
            assert!(err.contains("DX_CACHE_DIR"), "{err}");
            assert!(
                err.contains("bazel run @rules_dx//:dx -- <your arguments>"),
                "{err}"
            );
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_download_that_cannot_run_is_not_fetched_again() {
            let fx = Fixture::new("dx-launcher-noexec-mv-", true, None);
            fx.stub("mv", "#!/bin/sh\ncp \"$1\" \"$2\"\nchmod 644 \"$2\"\n");
            for _ in 0..2 {
                let (code, out, err) = fx.run(&["status"]);
                assert_eq!(code, Some(1), "{out}{err}");
                assert!(err.contains("noexec mount"), "{err}");
            }
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn a_swapped_binary_with_a_matching_sidecar_is_refused() {
            let fx = Fixture::new("dx-launcher-swap-", true, None);
            let original = fx.base.join("original");
            std::fs::write(&original, "#!/bin/sh\necho original\n").expect("original");
            let pin = sha_of(&original);
            fx.set_pin(&format!("1.2.3 {pin} dx-linux-x86_64\n"));
            let (code, out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(out.is_empty(), "{out}");
            assert!(err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains(&pin), "{err}");
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn an_empty_sha256_pin_runs_through_bazel_before_download() {
            let fx = Fixture::new("dx-launcher-nosha-", true, Some(""));
            fx.assert_via_bazel(&["check", "//..."]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn an_empty_sha256_pin_still_runs_a_cached_binary() {
            let fx = Fixture::new("dx-launcher-nosha-cached-", true, Some(""));
            let dir = fx.base.join("home/.cache/dx/bin/1.2.3");
            std::fs::create_dir_all(&dir).expect("cache dir");
            let bin = dir.join("dx-linux-x86_64");
            dx_testing::executable_script(&bin, PAYLOAD);
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn wrong_sha_refuses_to_run_and_caches_nothing() {
            let fx = Fixture::new("dx-launcher-sha-", true, Some(&"0".repeat(64)));
            let (code, out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1));
            assert!(out.is_empty(), "{out}");
            assert!(err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains("refusing to run"), "{err}");
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
        }

        #[test]
        fn offline_without_a_cached_binary_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-offline-", false, None);
            fx.assert_via_bazel(&["check", "//..."]);
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn a_module_bump_without_new_pins_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-bump-", true, None);
            fx.set_module("bazel_dep(name = \"rules_dx\", version = \"1.2.4\")\n");
            fx.assert_via_bazel(&["check", "//..."]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_module_bump_runs_direct_once_pins_are_refreshed() {
            let fx = Fixture::new("dx-launcher-refreshed-", true, None);
            fx.set_module("bazel_dep(name = \"rules_dx\", version = \"1.2.4\")\n");
            fx.assert_via_bazel(&["check", "//..."]);
            let sha = sha_of(&fx.base.join("payload"));
            fx.set_pin(&format!("1.2.4 {sha} dx-linux-x86_64\n"));
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 1);
            assert_eq!(fx.bazel_calls().len(), 1, "no new Bazel call");
        }

        #[test]
        fn a_missing_module_file_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-nomodule-", true, None);
            std::fs::remove_file(fx.base.join("repo/MODULE.bazel")).expect("remove module");
            fx.assert_via_bazel(&["check", "//..."]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn modules_the_launcher_cannot_read_run_through_bazel() {
            for (i, text) in [
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nlocal_path_override(module_name = \"rules_dx\", path = \"x\")\n",
                "bazel_dep(\n    name = \"rules_dx\",\n    version = \"1.2.3\",\n)\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\", repo_name = \"dx\")\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3&x\")\n",
            ]
            .into_iter()
            .enumerate()
            {
                let fx = Fixture::new(&format!("dx-launcher-module{i}-"), true, None);
                fx.set_module(text);
                fx.assert_via_bazel(&["check", "//..."]);
                assert_eq!(fx.downloads(), 0, "{text}");
            }
        }

        #[test]
        fn a_module_dep_with_trailing_cr_still_pins_the_version() {
            let fx = Fixture::new("dx-launcher-modulecr-", true, None);
            fx.set_module(
                "module(name = \"c\")\r\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")  \r\n",
            );
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert!(fx.bazel_calls().is_empty());
        }

        #[test]
        fn a_pins_file_for_another_version_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-oldpins-", true, None);
            let pin = sha_of(&fx.base.join("payload"));
            fx.set_pin(&format!("1.2.2 {pin} dx-linux-x86_64\n"));
            fx.assert_via_bazel(&["check", "//..."]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn curl_download_has_connect_and_stall_limits() {
            let fx = Fixture::new("dx-launcher-curl-limits-", false, None);
            let args = fx.base.join("curl-args");
            fx.stub(
                "curl",
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 6\n",
                    args.display()
                ),
            );
            fx.assert_via_bazel(&["version"]);
            let args = std::fs::read_to_string(&args).expect("curl args");
            let joined = args.lines().collect::<Vec<_>>().join(" ");
            assert!(
                joined
                    .contains("--connect-timeout 20 --speed-limit 1024 --speed-time 30 --retry 3"),
                "{joined}"
            );
            assert!(!joined.contains("--max-time"), "{joined}");
        }

        #[test]
        fn wget_download_has_timeout_and_tries_without_curl() {
            let fx = Fixture::new("dx-launcher-wget-limits-", false, None);
            std::fs::remove_file(fx.base.join("stub/curl")).expect("drop curl");
            let args = fx.base.join("wget-args");
            fx.stub(
                "wget",
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 8\n",
                    args.display()
                ),
            );
            fx.assert_via_bazel(&["version"]);
            let args = std::fs::read_to_string(&args).expect("wget args");
            let joined = args.lines().collect::<Vec<_>>().join(" ");
            assert!(joined.contains("--timeout=30 --tries=3"), "{joined}");
        }

        const UNAME_ROSETTA: &str =
            "#!/bin/sh\ncase \"$1\" in\n  -s) echo Darwin ;;\n  -m) echo x86_64 ;;\nesac\n";

        const ROSETTA_KEYS: [&str; 2] = ["hw.optional.arm64", "sysctl.proc_translated"];

        fn sysctl_stub(on: Option<&str>) -> String {
            let on = on.unwrap_or("none");
            format!(
                r#"#!/bin/sh
on='{on}'
n=0
if [ $# -eq 2 ] && [ "$1" = -n ]; then
  n=1
  shift
fi
if [ $# -ne 1 ]; then
  echo "sysctl: usage" >&2
  exit 1
fi
case "$1" in
  hw.optional.arm64 | sysctl.proc_translated) ;;
  *)
    echo "sysctl: unknown oid '$1'" >&2
    exit 1
    ;;
esac
if [ "$1" = "$on" ]; then v=1; else v=0; fi
if [ $n -eq 1 ]; then echo "$v"; else echo "$1: $v"; fi
"#
            )
        }

        #[test]
        fn rosetta_on_apple_silicon_runs_the_arm64_binary() {
            for (name, key) in [
                ("dx-launcher-rosetta-arm64-", ROSETTA_KEYS[0]),
                ("dx-launcher-rosetta-translated-", ROSETTA_KEYS[1]),
            ] {
                let fx = Fixture::new(name, true, None);
                fx.stub("uname", UNAME_ROSETTA);
                fx.stub("sysctl", &sysctl_stub(Some(key)));
                let (code, out, err) = fx.run(&["version"]);
                assert_eq!(
                    (code, out.as_str()),
                    (Some(0), "ran version\n"),
                    "{key}: {err}"
                );
                assert_eq!(fx.cached(), vec!["dx-macos-arm64"], "{key}");
            }
        }

        #[test]
        fn intel_mac_runs_through_bazel() {
            for (name, body) in [
                ("dx-launcher-intel-mac-", sysctl_stub(None)),
                (
                    "dx-launcher-intel-mac-broken-",
                    "#!/bin/sh\nexit 1\n".to_owned(),
                ),
            ] {
                let fx = Fixture::new(name, true, None);
                fx.stub("uname", UNAME_ROSETTA);
                fx.stub("sysctl", &body);
                fx.assert_via_bazel(&["version"]);
                assert_eq!(fx.downloads(), 0);
            }
        }

        #[test]
        fn rosetta_detection_needs_the_exact_key_and_dash_n() {
            let changes = [
                (ROSETTA_KEYS[0], "hw.optional.arm65", ROSETTA_KEYS[0]),
                (ROSETTA_KEYS[1], "sysctl.proc_translatd", ROSETTA_KEYS[1]),
                ("sysctl -n \"$key\"", "sysctl \"$key\"", ROSETTA_KEYS[0]),
            ];
            for (i, (from, to, on)) in changes.into_iter().enumerate() {
                let fx = Fixture::new(&format!("dx-launcher-rosetta-neg{i}-"), true, None);
                fx.stub("uname", UNAME_ROSETTA);
                fx.stub("sysctl", &sysctl_stub(Some(on)));
                fx.rewrite_launcher(from, to);
                fx.assert_via_bazel(&["version"]);
                assert_eq!(fx.downloads(), 0, "{from} -> {to}");
            }
        }

        #[test]
        fn msys_downloads_the_windows_binary_into_the_userprofile_cache() {
            let fx = Fixture::new("dx-launcher-msys-", true, None);
            fx.stub(
                "uname",
                "#!/bin/sh\ncase \"$1\" in -s) echo MINGW64_NT-10.0 ;; *) echo x86_64 ;; esac\n",
            );
            fx.stub("cygpath", "#!/bin/sh\necho \"$2\"\n");
            fx.stub("cmd.exe", "#!/bin/sh\necho cmd >> \"$STUB_LOG\"\n");
            let (code, out, err) = fx.run(&["a&b", "c|d", "^e", "%PATH%"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran a&b c|d ^e %PATH%\n"),
                "{err}"
            );
            assert_eq!(fx.downloads(), 1);
            assert_eq!(fx.cached(), vec!["dx-windows-x86_64.exe"]);
        }

        #[test]
        fn a_missing_pins_file_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-nopin-", true, None);
            std::fs::remove_file(fx.base.join("repo/.dx/pins")).expect("remove pin");
            let err = fx.assert_via_bazel(&["version"]);
            assert_eq!(
                err,
                "dx: no pinned binary for linux-x86_64 at 1.2.3, building from source through Bazel\n"
            );
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_failed_download_names_the_url_and_exit_code() {
            let fx = Fixture::new("dx-launcher-fetch-reason-", false, None);
            let err = fx.assert_via_bazel(&["version"]);
            assert_eq!(
                err,
                "dx: download of https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-linux-x86_64 failed (exit 6), building from source through Bazel\n"
            );
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn an_empty_pins_file_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-emptypin-", true, None);
            fx.set_pin("");
            fx.assert_via_bazel(&["version"]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn the_version_and_hash_come_from_the_pin_file() {
            let fx = Fixture::new("dx-launcher-pinfile-", true, None);
            let pin = sha_of(&fx.base.join("payload"));
            fx.set_pin(&format!("1.2.3 {pin} dx-linux-x86_64\r\n"));
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.cached(), vec!["dx-linux-x86_64"]);
        }
    }

    #[cfg(windows)]
    mod run_cmd {
        use super::*;
        use std::process::Command;

        fn arch() -> &'static str {
            if std::env::var("PROCESSOR_ARCHITECTURE")
                .is_ok_and(|a| a.eq_ignore_ascii_case("ARM64"))
            {
                "arm64"
            } else {
                "x86_64"
            }
        }

        fn sha_of(path: &Path) -> String {
            let out = Command::new("certutil")
                .arg("-hashfile")
                .arg(path)
                .arg("SHA256")
                .output()
                .expect("certutil");
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .nth(1)
                .expect("digest line")
                .replace(' ', "")
        }

        struct Fixture {
            _guard: Box<dyn std::any::Any>,
            base: std::path::PathBuf,
        }

        impl Fixture {
            fn new(name: &str, ps_ok: bool, sha: Option<&str>) -> Self {
                let dir = dx_test_scratch::scratch(name);
                let base = dir.path().to_path_buf();
                let root = base.join("repo");
                std::fs::create_dir_all(root.join(".dx")).expect("dx dir");
                std::fs::create_dir_all(base.join("stub")).expect("stub dir");
                std::fs::create_dir_all(base.join("home")).expect("home");
                std::fs::create_dir_all(base.join("calls")).expect("calls dir");
                let system = std::env::var("SystemRoot").expect("SystemRoot");
                let payload = base.join("payload");
                std::fs::copy(format!("{system}\\System32\\cmd.exe"), &payload).expect("payload");
                let real = sha_of(&payload);
                std::fs::write(base.join("sidecar"), format!("{real}  dx\r\n")).expect("sha");
                let want = sha.map_or(real, str::to_owned);
                let guid = "@echo off\r\nif defined DX_PART goto download\r\nif defined STUB_NO_GUID exit /b 0\r\n\"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" %*\r\nexit /b\r\n:download\r\necho %DX_PART%>\"%STUB_LOG%\\%DX_TAG%\"\r\n";
                let download = if ps_ok {
                    "copy /y \"%STUB_PAYLOAD%\" \"%DX_PART%\" >nul\r\ncopy /y \"%STUB_SIDECAR%\" \"%DX_PART%.sha\" >nul\r\nexit /b 0\r\n"
                } else {
                    "echo partial>\"%DX_PART%\"\r\nexit /b 9009\r\n"
                };
                std::fs::write(
                    base.join("stub/powershell.cmd"),
                    format!("{guid}{download}"),
                )
                .expect("stub");
                let want = want.to_ascii_lowercase();
                let pins = if want.is_empty() {
                    LauncherPins::unreleased()
                } else {
                    pinned(&want)
                };
                write_version_pins(&root, "1.2.3", &pins).expect("pins");
                std::fs::write(
                    root.join("MODULE.bazel"),
                    "module(name = \"consumer\")\r\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\r\n",
                )
                .expect("module");
                std::fs::write(
                    base.join("stub/bazel.cmd"),
                    "@echo off\r\necho %*>>\"%STUB_BAZEL%\"\r\n",
                )
                .expect("bazel stub");
                write_launcher_files(&root, &mut Applied::default()).expect("launcher");
                Self {
                    base,
                    _guard: Box::new(dir),
                }
            }

            fn set_module(&self, text: &str) {
                std::fs::write(self.base.join("repo/MODULE.bazel"), text).expect("module");
            }

            fn assert_via_bazel(&self, args: &[&str]) {
                let (code, out, err) = self.run(args);
                assert_eq!(code, Some(0), "{out}{err}");
                assert!(out.is_empty() && err.is_empty(), "{out}{err}");
                let calls =
                    std::fs::read_to_string(self.base.join("bazel.log")).unwrap_or_default();
                let want = format!("run @rules_dx//:dx -- {}", args.join(" "));
                assert_eq!(calls.trim(), want);
                assert!(self.cache_files().is_empty(), "{:?}", self.cache_files());
            }

            fn command(&self, args: &[&str]) -> Command {
                let base = &self.base;
                let system = std::env::var("SystemRoot").expect("SystemRoot");
                let mut cmd = Command::new("cmd");
                cmd.arg("/c")
                    .arg(base.join("repo/tools/dx.cmd"))
                    .args(args)
                    .env(
                        "PATH",
                        format!("{};{system}\\System32", base.join("stub").display()),
                    )
                    .env("USERPROFILE", base.join("home"))
                    .env("STUB_LOG", base.join("calls"))
                    .env("STUB_BAZEL", base.join("bazel.log"))
                    .env("STUB_PAYLOAD", base.join("payload"))
                    .env("STUB_SIDECAR", base.join("sidecar"))
                    .env_remove("DX_PART")
                    .env_remove("STUB_NO_GUID");
                cmd
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
                output(self.command(args).output().expect("launcher runs"))
            }

            fn part_names(&self) -> Vec<String> {
                std::fs::read_dir(self.base.join("calls"))
                    .expect("calls dir")
                    .flatten()
                    .map(|e| {
                        std::fs::read_to_string(e.path())
                            .expect("call log")
                            .trim()
                            .to_owned()
                    })
                    .collect()
            }

            fn downloads(&self) -> usize {
                self.part_names().len()
            }

            fn cache_files(&self) -> Vec<String> {
                std::fs::read_dir(self.base.join("home/.cache/dx/bin/1.2.3"))
                    .map(|entries| {
                        entries
                            .flatten()
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            }
        }

        fn output(out: std::process::Output) -> (Option<i32>, String, String) {
            (
                out.status.code(),
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        }

        #[test]
        fn downloads_once_then_only_runs() {
            let fx = Fixture::new("dx-launcher-cmd-run-", true, None);
            let (code, out, err) = fx.run(&["/c", "echo", "ran"]);
            assert_eq!(code, Some(0), "{out}{err}");
            assert!(out.contains("ran"), "{out}");
            assert_eq!(fx.downloads(), 1);
            assert_eq!(fx.cache_files(), vec![format!("dx-windows-{}.exe", arch())]);
            let (code, _out, err) = fx.run(&["/c", "echo", "again"]);
            assert_eq!(code, Some(0), "{err}");
            assert_eq!(fx.downloads(), 1, "cached run must not download");
        }

        #[test]
        fn wrong_sha_refuses_to_run_and_caches_nothing() {
            let fx = Fixture::new("dx-launcher-cmd-sha-", true, Some(&"0".repeat(64)));
            let (code, out, err) = fx.run(&["/c", "echo", "ran"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(!out.contains("ran"), "{out}");
            assert!(err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains("refusing to run"), "{err}");
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
        }

        #[test]
        fn a_swapped_binary_with_a_matching_sidecar_is_refused() {
            let fx = Fixture::new("dx-launcher-cmd-swap-", true, Some(&"b".repeat(64)));
            let (code, out, err) = fx.run(&["/c", "echo", "ran"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(!out.contains("ran"), "{out}");
            assert!(err.contains("sha256 mismatch"), "{err}");
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn an_empty_sha256_pin_runs_through_bazel_before_download() {
            let fx = Fixture::new("dx-launcher-cmd-nosha-", true, Some(""));
            fx.assert_via_bazel(&["check"]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn an_empty_sha256_pin_still_runs_a_cached_binary() {
            let fx = Fixture::new("dx-launcher-cmd-nosha-cached-", true, Some(""));
            let dir = fx.base.join("home/.cache/dx/bin/1.2.3");
            std::fs::create_dir_all(&dir).expect("cache dir");
            std::fs::copy(
                fx.base.join("payload"),
                dir.join(format!("dx-windows-{}.exe", arch())),
            )
            .expect("cached");
            let (code, out, err) = fx.run(&["/c", "echo", "ran"]);
            assert_eq!(code, Some(0), "{out}{err}");
            assert!(out.contains("ran"), "{out}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_failing_powershell_runs_through_bazel_and_leaves_no_part_file() {
            let fx = Fixture::new("dx-launcher-cmd-offline-", false, None);
            fx.assert_via_bazel(&["check"]);
        }

        #[test]
        fn concurrent_first_runs_use_distinct_part_files_and_all_succeed() {
            let fx = Fixture::new("dx-launcher-cmd-race-", true, None);
            let children: Vec<_> = (0..4)
                .map(|_| {
                    fx.command(&["/c", "echo", "ran"])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .expect("launcher starts")
                })
                .collect();
            for child in children {
                let (code, out, err) = output(child.wait_with_output().expect("launcher exits"));
                assert_eq!(code, Some(0), "{out}{err}");
                assert!(out.contains("ran"), "{out}{err}");
            }
            let mut parts = fx.part_names();
            assert_eq!(parts.len(), 4, "{parts:?}");
            parts.sort();
            parts.dedup();
            assert_eq!(
                parts.len(),
                4,
                "each run needs its own part file: {parts:?}"
            );
            assert_eq!(fx.cache_files(), vec![format!("dx-windows-{}.exe", arch())]);
        }

        #[test]
        fn no_guid_runs_through_bazel_and_caches_nothing() {
            let fx = Fixture::new("dx-launcher-cmd-noguid-", true, None);
            let mut cmd = fx.command(&["check"]);
            let (code, out, err) = output(
                cmd.env("STUB_NO_GUID", "1")
                    .output()
                    .expect("launcher runs"),
            );
            assert_eq!(code, Some(0), "{out}{err}");
            assert!(err.is_empty(), "{err}");
            assert_eq!(fx.downloads(), 0);
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
        }

        #[test]
        fn a_missing_pins_file_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-cmd-nopin-", true, None);
            std::fs::remove_file(fx.base.join("repo/.dx/pins")).expect("remove pin");
            fx.assert_via_bazel(&["version"]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_module_bump_without_new_pins_runs_through_bazel() {
            let fx = Fixture::new("dx-launcher-cmd-bump-", true, None);
            fx.set_module("bazel_dep(name = \"rules_dx\", version = \"1.2.4\")\r\n");
            fx.assert_via_bazel(&["check"]);
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn modules_the_launcher_cannot_read_run_through_bazel() {
            for (i, text) in [
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\r\nlocal_path_override(module_name = \"rules_dx\", path = \"x\")\r\n",
                "bazel_dep(\r\n    name = \"rules_dx\",\r\n    version = \"1.2.3\",\r\n)\r\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\", repo_name = \"dx\")\r\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\r\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\r\n",
                "bazel_dep(name = \"rules_dx\", version = \"1.2.3&x\")\r\n",
            ]
            .into_iter()
            .enumerate()
            {
                let fx = Fixture::new(&format!("dx-launcher-cmd-module{i}-"), true, None);
                fx.set_module(text);
                fx.assert_via_bazel(&["check"]);
                assert_eq!(fx.downloads(), 0, "{text}");
            }
        }
    }
}
