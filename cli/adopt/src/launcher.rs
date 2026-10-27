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

/// The sha256 of each release asset, baked into the rendered launchers.
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
        for line in text.lines() {
            let Some((hash, asset)) = line.split_once("  ") else {
                return Err(pins_error(format!("bad manifest line {line:?}")));
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
        if let Some(i) = pins.sha256.iter().position(String::is_empty) {
            return Err(pins_error(format!("{} is missing", RELEASE_ASSETS[i])));
        }
        Ok(pins)
    }
}

/// The sha256 pins for dx `version`'s release assets.
///
/// Always unreleased (empty) for now: no dx release exists yet. Delivering
/// real pins through the release source archive is the follow-up to #2294.
pub fn release_pins(_version: &str) -> Result<LauncherPins, AdoptError> {
    Ok(LauncherPins::unreleased())
}

fn valid_pin(pin: &str) -> bool {
    pin.is_empty()
        || (pin.len() == 64 && pin.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

fn pin_var(asset: &str) -> String {
    format!(
        "SHA256_{}",
        asset
            .trim_start_matches("dx-")
            .replace('-', "_")
            .to_uppercase()
    )
}

fn pins_error(detail: String) -> AdoptError {
    AdoptError::LauncherPins { detail }
}

const LAUNCHER_SH: &str = r##"#!/bin/sh
set -eu
VERSION="@VERSION@"
SHA256_LINUX_X86_64="@SHA256_LINUX_X86_64@"
SHA256_LINUX_ARM64="@SHA256_LINUX_ARM64@"
SHA256_MACOS_ARM64="@SHA256_MACOS_ARM64@"
SHA256_WINDOWS_X86_64="@SHA256_WINDOWS_X86_64@"
SHA256_WINDOWS_ARM64="@SHA256_WINDOWS_ARM64@"
args="$*"
fail() {
  echo "dx: $1" >&2
  echo "dx: run it through Bazel instead: bazel run @rules_dx//:dx -- $args" >&2
  exit 1
}
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
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
pin=
[ -f "$root/.dx/version" ] || fail "tools/dx is pinned to dx $VERSION but .dx/version is missing; refresh it: bazel run @rules_dx//:dx -- init --apply"
IFS= read -r pin < "$root/.dx/version" || true
pin=$(printf %s "$pin" | tr -d '\r ')
[ "$pin" = "$VERSION" ] || fail "tools/dx is pinned to dx $VERSION but .dx/version is '$pin'; refresh it: bazel run @rules_dx//:dx -- init --apply"
[ -n "$os" ] || fail "no dx binary for $(uname -s)"
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) fail "no dx binary for $(uname -m)" ;;
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
  linux-x86_64) want=$SHA256_LINUX_X86_64 ;;
  linux-arm64) want=$SHA256_LINUX_ARM64 ;;
  macos-arm64) want=$SHA256_MACOS_ARM64 ;;
  windows-x86_64) want=$SHA256_WINDOWS_X86_64 ;;
  windows-arm64) want=$SHA256_WINDOWS_ARM64 ;;
  *) fail "no dx binary for $os $arch" ;;
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
if [ -x "$bin" ]; then
  exec "$bin" "$@"
fi
[ -n "$want" ] || fail "tools/dx has no pinned sha256 for dx-$os-$arch; rerun dx init --apply"
url="https://github.com/devhusdk/rules_dx/releases/download/v$VERSION/dx-$os-$arch"
mkdir -p "$dir"
tmp="$dir/.dx-$os-$arch.$$"
trap 'rm -f "$tmp"' EXIT
fetch() {
  if command -v curl > /dev/null 2>&1; then
    curl -fsSL -o "$2" "$1" || fail "download failed: $1"
  elif command -v wget > /dev/null 2>&1; then
    wget -q -O "$2" "$1" || fail "download failed: $1"
  else
    fail "needs curl or wget to download $1"
  fi
}
fetch "$url" "$tmp"
if command -v sha256sum > /dev/null 2>&1; then
  got=$(sha256sum "$tmp" | cut -d' ' -f1)
elif command -v shasum > /dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
else
  fail "needs sha256sum or shasum"
fi
[ "$got" = "$want" ] || fail "sha256 mismatch for $url: want '$want', got $got; refusing to run it"
chmod +x "$tmp"
mv "$tmp" "$bin"
exec "$bin" "$@"
"##;

const LAUNCHER_CMD: &str = r##"@echo off
setlocal
set "VERSION=@VERSION@"
set "SHA256_WINDOWS_X86_64=@SHA256_WINDOWS_X86_64@"
set "SHA256_WINDOWS_ARM64=@SHA256_WINDOWS_ARM64@"
set "ARCH=x86_64"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ARCH=arm64"
set "WANT="
if "%ARCH%"=="arm64" (set "WANT=%SHA256_WINDOWS_ARM64%") else set "WANT=%SHA256_WINDOWS_X86_64%"
set "PIN="
if not exist "%~dp0..\.dx\version" (
  echo dx: tools\dx.cmd is pinned to dx %VERSION% but .dx\version is missing; refresh it: bazel run @rules_dx//:dx -- init --apply 1>&2
  goto bazel
)
set /p PIN=<"%~dp0..\.dx\version"
if not "%PIN%"=="%VERSION%" (
  echo dx: tools\dx.cmd is pinned to dx %VERSION% but .dx\version is '%PIN%'; refresh it: bazel run @rules_dx//:dx -- init --apply 1>&2
  goto bazel
)
set "DX_CACHE=%USERPROFILE%\.cache\dx"
if defined XDG_CACHE_HOME set "DX_CACHE=%XDG_CACHE_HOME%\dx"
if defined DX_CACHE_DIR set "DX_CACHE=%DX_CACHE_DIR%"
set "DIR=%DX_CACHE%\bin\%VERSION%"
set "BIN=%DIR%\dx-windows-%ARCH%.exe"
if exist "%BIN%" goto run
if not defined WANT (
  echo dx: tools\dx.cmd has no pinned sha256 for dx-windows-%ARCH%; rerun dx init --apply 1>&2
  goto bazel
)
set "DX_URL=https://github.com/devhusdk/rules_dx/releases/download/v%VERSION%/dx-windows-%ARCH%"
mkdir "%DIR%" 2>nul
set "DX_TAG="
for /f %%G in ('powershell -NoProfile -NonInteractive -Command "[guid]::NewGuid().ToString()"') do set "DX_TAG=%%G"
if not defined DX_TAG (
  echo dx: download failed: %DX_URL% 1>&2
  goto bazel
)
set "DX_PART=%DIR%\.dx-%ARCH%.%DX_TAG%.part"
call powershell -NoProfile -NonInteractive -Command "$ErrorActionPreference='Stop'; Invoke-WebRequest -UseBasicParsing -Uri $env:DX_URL -OutFile $env:DX_PART"
if errorlevel 1 (
  call :clean
  echo dx: download failed: %DX_URL% 1>&2
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
goto bazel
:run
"%BIN%" %*
exit /b %errorlevel%
:mismatch
call :clean
echo dx: sha256 mismatch for %DX_URL%: want '%WANT%', got '%GOT%'; refusing to run it 1>&2
goto bazel
:clean
del /q "%DX_PART%" 2>nul
exit /b 0
:bazel
echo dx: run it through Bazel instead: bazel run @rules_dx//:dx -- %* 1>&2
exit /b 1
"##;

pub fn render_launcher_sh(version: &str, pins: &LauncherPins) -> Result<String, AdoptError> {
    render(LAUNCHER_SH, version, pins, &RELEASE_ASSETS)
}

pub fn render_launcher_cmd(version: &str, pins: &LauncherPins) -> Result<String, AdoptError> {
    Ok(render(
        LAUNCHER_CMD,
        version,
        pins,
        &["dx-windows-x86_64", "dx-windows-arm64"],
    )?
    .replace('\n', "\r\n"))
}

fn render(
    template: &str,
    version: &str,
    pins: &LauncherPins,
    assets: &[&str],
) -> Result<String, AdoptError> {
    let mut text = template.replace("@VERSION@", version);
    for asset in assets {
        let pin = pins.get(asset).unwrap_or_default();
        if !valid_pin(pin) {
            return Err(pins_error(format!(
                "{asset} pin is not 64 lowercase hex: {pin:?}"
            )));
        }
        text = text.replace(&format!("@{}@", pin_var(asset)), pin);
    }
    if text.contains("@SHA256_") {
        return Err(pins_error(
            "launcher template has an unfilled sha256 pin".to_owned(),
        ));
    }
    Ok(text)
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
pub fn write_launcher_files(
    root: &Path,
    version: &str,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    sync(root, version, true, true, applied)
}

pub fn refresh_launcher_files(root: &Path, version: &str) -> Result<Vec<String>, AdoptError> {
    let mut applied = Applied::default();
    sync(root, version, false, true, &mut applied)?;
    Ok(applied.wrote)
}

pub fn plan_launcher_files(
    root: &Path,
    version: &str,
    create_missing: bool,
) -> Result<Vec<String>, AdoptError> {
    let mut applied = Applied::default();
    sync(root, version, create_missing, false, &mut applied)?;
    Ok(applied.wrote)
}

pub fn launcher_version(root: &Path, rel: &str) -> Option<String> {
    let dest = crate::scaffold::scaffold_dest_within_root(root, rel).ok()?;
    let text = std::fs::read_to_string(dest).ok()?;
    let mut lines = text.lines().map(|line| line.trim_end_matches('\r'));
    let prefix = match (lines.next()?, lines.next()?) {
        ("#!/bin/sh", "set -eu") => "VERSION=\"",
        ("@echo off", "setlocal") => "set \"VERSION=",
        _ => return None,
    };
    lines
        .next()?
        .strip_prefix(prefix)?
        .strip_suffix('"')
        .map(str::to_owned)
}

pub fn stale_launchers(root: &Path, version: &str) -> Vec<(String, String)> {
    [LAUNCHER_SH_REL, LAUNCHER_CMD_REL]
        .into_iter()
        .filter_map(|rel| {
            launcher_version(root, rel)
                .filter(|found| found != version)
                .map(|found| (rel.to_owned(), found))
        })
        .collect()
}

fn sync(
    root: &Path,
    version: &str,
    create_missing: bool,
    apply: bool,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    let mut plan = Vec::new();
    for rel in [LAUNCHER_SH_REL, LAUNCHER_CMD_REL] {
        let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
        let write = if dest.exists() {
            launcher_version(root, rel).is_some_and(|found| found != version)
        } else {
            create_missing
        };
        plan.push((rel, write));
    }
    if plan.iter().any(|(_, write)| *write) && semver::Version::parse(version).is_err() {
        return Err(AdoptError::LauncherVersion {
            version: version.to_owned(),
        });
    }
    let pins = if apply && plan.iter().any(|(_, write)| *write) {
        release_pins(version)?
    } else {
        LauncherPins::unreleased()
    };
    for (rel, write) in plan {
        if !write {
            applied.refused.push(rel.to_owned());
            continue;
        }
        if apply {
            let (content, executable) = if rel == LAUNCHER_SH_REL {
                (render_launcher_sh(version, &pins)?, true)
            } else {
                (render_launcher_cmd(version, &pins)?, false)
            };
            write_launcher(root, rel, &content, executable)?;
        }
        applied.wrote.push(rel.to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinned(sha: &str) -> LauncherPins {
        LauncherPins {
            sha256: std::array::from_fn(|_| sha.to_owned()),
        }
    }

    fn sh(version: &str) -> String {
        render_launcher_sh(version, &LauncherPins::unreleased()).expect("render sh")
    }

    fn cmd(version: &str) -> String {
        render_launcher_cmd(version, &LauncherPins::unreleased()).expect("render cmd")
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
        let sh = sh("1.2.3");
        let cmd = cmd("1.2.3");
        for script in [&sh, &cmd] {
            for gone in [
                "bazel-bin",
                ".dx/launcher",
                ".dx\\launcher",
                "//:dx\"",
                "build //:dx",
                "rules_dx\",",
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
        let pin = sh.find("\nIFS= read -r pin").expect("pin check");
        assert!(os < msys && msys < pin);
    }
    #[test]
    fn launcher_pins_version_and_hashes() {
        let pins = LauncherPins::from_sha256sums(MANIFEST).expect("manifest");
        let sh = render_launcher_sh("1.2.3", &pins).expect("sh");
        assert!(sh.starts_with("#!/bin/sh\n"));
        assert!(sh.contains("VERSION=\"1.2.3\""));
        assert!(!sh.contains("@VERSION@"));
        for (n, asset) in RELEASE_ASSETS.iter().enumerate() {
            let line = format!(
                "{}=\"{}\"\n",
                pin_var(asset),
                (n + 1).to_string().repeat(64)
            );
            assert_eq!(sh.matches(&line).count(), 1, "{line}");
        }
        assert_eq!(sh.lines().filter(|l| l.starts_with("SHA256_")).count(), 5);
        let cmd = render_launcher_cmd("1.2.3", &pins).expect("cmd");
        assert!(cmd.contains("set \"VERSION=1.2.3\"\r\n"));
        assert!(cmd.contains(&format!(
            "set \"SHA256_WINDOWS_X86_64={}\"\r\n",
            "4".repeat(64)
        )));
        assert!(cmd.contains(&format!(
            "set \"SHA256_WINDOWS_ARM64={}\"\r\n",
            "5".repeat(64)
        )));
        assert_eq!(
            cmd.lines()
                .filter(|l| l.starts_with("set \"SHA256_"))
                .count(),
            2
        );
        assert!(!cmd.contains("@VERSION@"));
        assert!(!cmd.replace("\r\n", "").contains('\n'));
        for script in [&sh, &cmd] {
            for gone in ["@SHA256_", ".sha256\"", "'.sha256'", ".sha\""] {
                assert!(!script.contains(gone), "{gone}");
            }
        }
    }

    #[test]
    fn launcher_version_still_reads_line_three() {
        let dir = dx_test_scratch::scratch("dx-launcher-line3-");
        let pins = pinned(&"a".repeat(64));
        std::fs::create_dir_all(dir.path().join("tools")).expect("tools");
        let sh = render_launcher_sh("1.2.3", &pins).expect("sh");
        let cmd = render_launcher_cmd("1.2.3", &pins).expect("cmd");
        assert_eq!(sh.lines().nth(2), Some("VERSION=\"1.2.3\""));
        assert_eq!(cmd.lines().nth(2), Some("set \"VERSION=1.2.3\""));
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), sh).expect("sh");
        std::fs::write(dir.path().join(LAUNCHER_CMD_REL), cmd).expect("cmd");
        for rel in [LAUNCHER_SH_REL, LAUNCHER_CMD_REL] {
            assert_eq!(launcher_version(dir.path(), rel).as_deref(), Some("1.2.3"));
        }
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

    #[test]
    fn render_refuses_an_invalid_pin() {
        for pin in [
            "A".repeat(64),
            "a".repeat(63),
            "\"; rm -rf /; \"".to_owned(),
        ] {
            let pins = pinned(&pin);
            assert!(matches!(
                render_launcher_sh("1.2.3", &pins),
                Err(AdoptError::LauncherPins { .. })
            ));
            assert!(matches!(
                render_launcher_cmd("1.2.3", &pins),
                Err(AdoptError::LauncherPins { .. })
            ));
        }
    }

    #[test]
    fn release_pins_of_unreleased_is_empty() {
        let pins = release_pins(crate::DX_VERSION).expect("pins");
        assert_eq!(pins, LauncherPins::unreleased());
        for asset in RELEASE_ASSETS {
            assert_eq!(pins.get(asset), Some(""));
        }
        let sh = sh("1.2.3");
        assert!(sh.contains("SHA256_LINUX_X86_64=\"\"\n"));
    }

    #[test]
    fn launchers_share_the_shim_cache_root() {
        let sh = render_launcher_sh("1.2.3");
        let dx = sh.find("cache=\"$DX_CACHE_DIR\"").expect("DX_CACHE_DIR");
        let xdg = sh
            .find("cache=\"$XDG_CACHE_HOME/dx\"")
            .expect("XDG_CACHE_HOME");
        let home = sh.find("cache=\"$HOME/.cache/dx\"").expect("HOME");
        assert!(dx < xdg && xdg < home);
        assert!(sh.contains("dir=\"$cache/bin/$VERSION\""));
        let cmd = render_launcher_cmd("1.2.3");
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
        let first = crate::scaffold::entries(|a| write_launcher_files(dir.path(), "1.2.3", a))
            .expect("writes");
        assert_eq!(first, vec!["tools/dx", "tools/dx.cmd"]);
        let current = crate::scaffold::entries(|a| write_launcher_files(dir.path(), "1.2.3", a))
            .expect("current");
        assert_eq!(current, vec!["refused:tools/dx", "refused:tools/dx.cmd"]);
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), "custom\n").expect("edit");
        let second = crate::scaffold::entries(|a| write_launcher_files(dir.path(), "1.3.0", a))
            .expect("syncs");
        assert_eq!(second, vec!["tools/dx.cmd", "refused:tools/dx"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(LAUNCHER_SH_REL)).expect("read"),
            "custom\n"
        );
        assert_eq!(
            launcher_version(dir.path(), LAUNCHER_CMD_REL).as_deref(),
            Some("1.3.0")
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
    fn refresh_only_rewrites_existing_stale_launchers() {
        let dir = dx_test_scratch::scratch("dx-launcher-refresh-");
        assert!(refresh_launcher_files(dir.path(), "1.2.3")
            .expect("none")
            .is_empty());
        assert!(!dir.path().join(LAUNCHER_SH_REL).exists());
        std::fs::create_dir_all(dir.path().join("tools")).expect("tools");
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), sh("1.2.3")).expect("seed");
        assert!(refresh_launcher_files(dir.path(), "1.2.3")
            .expect("current")
            .is_empty());
        assert_eq!(
            stale_launchers(dir.path(), "1.3.0"),
            vec![("tools/dx".to_owned(), "1.2.3".to_owned())]
        );
        let wrote = refresh_launcher_files(dir.path(), "1.3.0").expect("refresh");
        assert_eq!(wrote, vec!["tools/dx"]);
        assert!(!dir.path().join(LAUNCHER_CMD_REL).exists());
        assert!(stale_launchers(dir.path(), "1.3.0").is_empty());
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

    #[test]
    fn launcher_version_reads_both_templates() {
        let dir = dx_test_scratch::scratch("dx-launcher-version-");
        std::fs::create_dir_all(dir.path().join("tools")).expect("tools");
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), sh("1.2.3")).expect("sh");
        std::fs::write(dir.path().join(LAUNCHER_CMD_REL), cmd("2.0.0-rc.1")).expect("cmd");
        assert_eq!(
            launcher_version(dir.path(), LAUNCHER_SH_REL).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            launcher_version(dir.path(), LAUNCHER_CMD_REL).as_deref(),
            Some("2.0.0-rc.1")
        );
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
        assert_eq!(launcher_version(dir.path(), LAUNCHER_SH_REL), None);
        assert_eq!(launcher_version(dir.path(), LAUNCHER_CMD_REL), None);
        assert!(stale_launchers(dir.path(), "9.9.9").is_empty());
        assert_eq!(launcher_version(dir.path(), "tools/missing"), None);
        std::fs::write(
            dir.path().join(LAUNCHER_SH_REL),
            "#!/bin/sh\nVERSION=\"$(cat .dx/version)\"\nexec dx \"$@\"\n",
        )
        .expect("custom with VERSION");
        assert_eq!(launcher_version(dir.path(), LAUNCHER_SH_REL), None);
    }

    #[test]
    fn a_non_semver_version_writes_nothing() {
        let dir = dx_test_scratch::scratch("dx-launcher-semver-");
        for bad in ["1\"; x", "../1", ""] {
            let refused = AdoptError::LauncherVersion {
                version: bad.to_owned(),
            };
            assert_eq!(
                write_launcher_files(dir.path(), bad, &mut Applied::default()),
                Err(refused.clone())
            );
            assert_eq!(plan_launcher_files(dir.path(), bad, true), Err(refused));
            assert_eq!(refresh_launcher_files(dir.path(), bad), Ok(Vec::new()));
        }
        assert!(!dir.path().join("tools").exists());
        write_launcher_files(dir.path(), "1.2.3", &mut Applied::default()).expect("seed");
        assert!(refresh_launcher_files(dir.path(), "v1").is_err());
        assert_eq!(
            launcher_version(dir.path(), LAUNCHER_SH_REL).as_deref(),
            Some("1.2.3")
        );
        for rel in [LAUNCHER_SH_REL, LAUNCHER_CMD_REL] {
            std::fs::write(dir.path().join(rel), "custom\n").expect("custom");
        }
        assert_eq!(
            crate::scaffold::entries(|a| write_launcher_files(dir.path(), "v1", a))
                .expect("custom kept"),
            vec!["refused:tools/dx", "refused:tools/dx.cmd"]
        );
    }

    #[test]
    fn plan_reports_writes_without_writing() {
        let dir = dx_test_scratch::scratch("dx-launcher-plan-");
        assert_eq!(
            plan_launcher_files(dir.path(), "1.2.3", true).expect("plan"),
            vec!["tools/dx", "tools/dx.cmd"]
        );
        assert!(plan_launcher_files(dir.path(), "1.2.3", false)
            .expect("plan")
            .is_empty());
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
                std::fs::write(root.join(".dx/version"), "1.2.3\n").expect("pin");
                std::fs::create_dir_all(dir.path().join("stub")).expect("stub dir");
                std::fs::create_dir_all(dir.path().join("home")).expect("home");
                let tools = dir.path().join("tools");
                let missing = dx_test_scratch::link_host_tools(
                    &tools,
                    &["dirname", "tr", "mkdir", "cut", "chmod", "mv", "rm", "cp"],
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
                let curl = dir.path().join("stub/curl");
                let body = if curl_ok {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; fi\n  url=\"$1\"\n  shift\ndone\ncase \"$url\" in\n  *.sha256) cp \"$STUB_SIDECAR\" \"$out\" ;;\n  *) cp \"$STUB_PAYLOAD\" \"$out\" ;;\nesac\n"
                } else {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nexit 6\n"
                };
                std::fs::write(&curl, body).expect("curl");
                std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
                write_launcher_files(&root, "1.2.3", &mut Applied::default()).expect("launcher");
                let launcher = root.join(LAUNCHER_SH_REL);
                std::fs::write(
                    &launcher,
                    render_launcher_sh("1.2.3", &pinned(&pin)).expect("render"),
                )
                .expect("pinned launcher");
                std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
                let fx = Self {
                    base: dir.path().to_path_buf(),
                    _guard: Box::new(dir),
                };
                fx.stub("uname", UNAME_LINUX);
                fx
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
                let base = &self.base;
                let out = spawn(
                    Command::new(base.join("repo/tools/dx"))
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
                std::fs::write(&path, body).expect("stub");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
            }

            fn rewrite_launcher(&self, from: &str, to: &str) {
                let path = self.base.join("repo/tools/dx");
                let text = std::fs::read_to_string(&path).expect("read launcher");
                assert!(text.contains(from), "launcher template lost {from:?}");
                std::fs::write(&path, text.replacen(from, to, 1)).expect("write launcher");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
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

        #[test]
        fn downloads_once_then_only_execs() {
            let fx = Fixture::new("dx-launcher-run-", true, None);
            let (code, out, err) = fx.run(&["version", "--check"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran version --check\n"),
                "{err}"
            );
            assert_eq!(fx.downloads(), 1, "the sidecar is never fetched");
            assert_eq!(fx.cached(), vec!["dx-linux-x86_64"]);
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 1, "cached run must not download");
        }

        #[test]
        fn a_swapped_binary_with_a_matching_sidecar_is_refused() {
            let fx = Fixture::new("dx-launcher-swap-", true, None);
            let original = fx.base.join("original");
            std::fs::write(&original, "#!/bin/sh\necho original\n").expect("original");
            let pin = sha_of(&original);
            fx.rewrite_launcher(&sha_of(&fx.base.join("payload")), &pin);
            let (code, out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(out.is_empty(), "{out}");
            assert!(err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains(&pin), "{err}");
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
            assert_eq!(fx.downloads(), 1);
        }

        #[test]
        fn an_empty_sha256_pin_refuses_before_download() {
            let fx = Fixture::new("dx-launcher-nosha-", true, Some(""));
            let (code, out, err) = fx.run(&["check", "//..."]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(out.is_empty(), "{out}");
            assert!(
                err.contains("no pinned sha256 for dx-linux-x86_64"),
                "{err}"
            );
            assert!(
                err.contains("bazel run @rules_dx//:dx -- check //..."),
                "{err}"
            );
            assert_eq!(fx.downloads(), 0);
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
        }

        #[test]
        fn an_empty_sha256_pin_still_runs_a_cached_binary() {
            let fx = Fixture::new("dx-launcher-nosha-cached-", true, Some(""));
            let dir = fx.base.join("home/.cache/dx/bin/1.2.3");
            std::fs::create_dir_all(&dir).expect("cache dir");
            let bin = dir.join("dx-linux-x86_64");
            std::fs::write(&bin, PAYLOAD).expect("cached");
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
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
        fn offline_without_a_cached_binary_prints_the_bazel_command() {
            let fx = Fixture::new("dx-launcher-offline-", false, None);
            let (code, out, err) = fx.run(&["check", "//..."]);
            assert_eq!(code, Some(1));
            assert!(out.is_empty(), "{out}");
            assert!(err.contains("download failed"), "{err}");
            assert!(
                err.contains("bazel run @rules_dx//:dx -- check //..."),
                "{err}"
            );
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
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
        fn intel_mac_still_has_no_binary() {
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
                let (code, out, err) = fx.run(&["version"]);
                assert_eq!(code, Some(1), "{out}{err}");
                assert!(err.contains("no dx binary for macos x86_64"), "{err}");
                assert_eq!(fx.downloads(), 0);
                assert!(fx.cached().is_empty(), "{:?}", fx.cached());
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
                let (code, out, err) = fx.run(&["version"]);
                assert_eq!(code, Some(1), "{from} -> {to}: {out}{err}");
                assert!(
                    err.contains("no dx binary for macos x86_64"),
                    "{from} -> {to}: {err}"
                );
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

        fn assert_clean_missing_pin(fx: &Fixture, err: &str, code: Option<i32>) {
            assert_eq!(code, Some(1), "{err}");
            assert_eq!(err.lines().count(), 2, "{err}");
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx/version is missing"),
                "{err}"
            );
            assert!(err.contains("init --apply"), "{err}");
            assert!(err.contains("bazel run @rules_dx//:dx -- version"), "{err}");
            for noise in [
                "parameter not set",
                "cannot open",
                "can't open",
                "No such file",
            ] {
                assert!(!err.contains(noise), "{noise}: {err}");
            }
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_missing_pin_is_refused_with_one_message() {
            let fx = Fixture::new("dx-launcher-nopin-", true, None);
            std::fs::remove_file(fx.base.join("repo/.dx/version")).expect("remove pin");
            let (code, out, err) = fx.run(&["version"]);
            assert!(out.is_empty(), "{out}");
            assert_clean_missing_pin(&fx, &err, code);
        }

        #[test]
        fn an_empty_pin_is_refused() {
            let fx = Fixture::new("dx-launcher-emptypin-", true, None);
            std::fs::write(fx.base.join("repo/.dx/version"), "").expect("pin");
            let (code, _out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1), "{err}");
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx/version is ''"),
                "{err}"
            );
            assert!(!err.contains("parameter not set"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_pin_that_differs_from_the_launcher_is_refused() {
            let fx = Fixture::new("dx-launcher-pin-", true, None);
            std::fs::write(fx.base.join("repo/.dx/version"), "2.0.0\n").expect("pin");
            let (code, _out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1));
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx/version is '2.0.0'"),
                "{err}"
            );
            assert!(err.contains("init --apply"), "{err}");
            assert_eq!(fx.downloads(), 0);
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
                std::fs::write(root.join(".dx/version"), "1.2.3\r\n").expect("pin");
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
                write_launcher_files(&root, "1.2.3", &mut Applied::default()).expect("launcher");
                std::fs::write(
                    root.join(LAUNCHER_CMD_REL),
                    render_launcher_cmd("1.2.3", &pinned(&want.to_ascii_lowercase()))
                        .expect("render"),
                )
                .expect("pinned launcher");
                Self {
                    base,
                    _guard: Box::new(dir),
                }
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
        fn an_empty_sha256_pin_refuses_before_download() {
            let fx = Fixture::new("dx-launcher-cmd-nosha-", true, Some(""));
            let (code, out, err) = fx.run(&["check"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(err.contains("no pinned sha256"), "{err}");
            assert!(!err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains("bazel run @rules_dx//:dx -- check"), "{err}");
            assert_eq!(fx.downloads(), 0);
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
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
        fn a_failing_powershell_is_a_download_failure_and_leaves_no_part_file() {
            let fx = Fixture::new("dx-launcher-cmd-offline-", false, None);
            let (code, out, err) = fx.run(&["check"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(err.contains("download failed"), "{err}");
            assert!(!err.contains("sha256 mismatch"), "{err}");
            assert!(err.contains("bazel run @rules_dx//:dx -- check"), "{err}");
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
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
        fn no_guid_is_a_download_failure_and_caches_nothing() {
            let fx = Fixture::new("dx-launcher-cmd-noguid-", true, None);
            let mut cmd = fx.command(&["check"]);
            let (code, out, err) = output(
                cmd.env("STUB_NO_GUID", "1")
                    .output()
                    .expect("launcher runs"),
            );
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(err.contains("download failed"), "{err}");
            assert!(err.contains("bazel run @rules_dx//:dx -- check"), "{err}");
            assert_eq!(fx.downloads(), 0);
            assert!(fx.cache_files().is_empty(), "{:?}", fx.cache_files());
        }

        #[test]
        fn a_missing_pin_is_refused_without_a_file_error() {
            let fx = Fixture::new("dx-launcher-cmd-nopin-", true, None);
            std::fs::remove_file(fx.base.join("repo/.dx/version")).expect("remove pin");
            let (code, _out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1));
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx\\version is missing"),
                "{err}"
            );
            assert!(!err.contains("cannot find the file"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }

        #[test]
        fn a_pin_that_differs_from_the_launcher_is_refused() {
            let fx = Fixture::new("dx-launcher-cmd-pin-", true, None);
            std::fs::write(fx.base.join("repo/.dx/version"), "2.0.0\r\n").expect("pin");
            let (code, _out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1));
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx\\version is '2.0.0'"),
                "{err}"
            );
            assert!(err.contains("init --apply"), "{err}");
            assert_eq!(fx.downloads(), 0);
        }
    }
}
