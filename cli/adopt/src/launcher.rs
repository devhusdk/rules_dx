use std::path::Path;

use super::AdoptError;

pub const LAUNCHER_SH_REL: &str = "tools/dx";

pub const LAUNCHER_CMD_REL: &str = "tools/dx.cmd";

const LAUNCHER_SH: &str = r##"#!/bin/sh
set -eu
VERSION="@VERSION@"
args="$*"
fail() {
  echo "dx: $1" >&2
  echo "dx: run it through Bazel instead: bazel run @rules_dx//:dx -- $args" >&2
  exit 1
}
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    exec env MSYS2_ARG_CONV_EXCL='*' cmd.exe /c "$(cygpath -w "$root/tools/dx.cmd")" "$@"
    ;;
esac
if [ -f "$root/cli/cli/BUILD.bazel" ] && grep -q '^    name = "rules_dx",' "$root/MODULE.bazel" 2> /dev/null; then
  bin="$root/bazel-bin/cli/cli/dx"
  if [ ! -x "$bin" ] || [ -n "$(find "$root/cli" -name target -prune -o -type f \( -name '*.rs' -o -name '*.bazel' \) -newer "$bin" -print -quit 2> /dev/null)" ]; then
    command -v bazel > /dev/null 2>&1 || fail "bazel is not on PATH"
    (cd "$root" && bazel build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx >&2) || fail "bazel build //:dx failed"
  fi
  exec "$bin" "$@"
fi
IFS= read -r pin < "$root/.dx/version" || true
pin=$(printf %s "$pin" | tr -d '\r ')
[ "$pin" = "$VERSION" ] || fail "tools/dx is pinned to dx $VERSION but .dx/version is '$pin'; run dx upgrade"
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fail "no dx binary for $(uname -s)" ;;
esac
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
  linux-x86_64 | linux-arm64 | macos-arm64) ;;
  *) fail "no dx binary for $os $arch" ;;
esac
[ -n "${HOME:-}" ] || fail "HOME is not set"
dir="$HOME/.cache/dx/bin/$VERSION"
bin="$dir/dx-$os-$arch"
if [ -x "$bin" ]; then
  exec "$bin" "$@"
fi
url="https://github.com/devhusdk/rules_dx/releases/download/v$VERSION/dx-$os-$arch"
mkdir -p "$dir"
tmp="$dir/.dx-$os-$arch.$$"
trap 'rm -f "$tmp" "$tmp.sha256"' EXIT
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
fetch "$url.sha256" "$tmp.sha256"
IFS=' ' read -r want _ < "$tmp.sha256" || true
if command -v sha256sum > /dev/null 2>&1; then
  got=$(sha256sum "$tmp" | cut -d' ' -f1)
elif command -v shasum > /dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
else
  fail "needs sha256sum or shasum"
fi
[ -n "$want" ] && [ "$got" = "$want" ] || fail "sha256 mismatch for $url: want '$want', got $got; refusing to run it"
chmod +x "$tmp"
mv "$tmp" "$bin"
rm -f "$tmp.sha256"
exec "$bin" "$@"
"##;

const LAUNCHER_CMD: &str = r##"@echo off
setlocal
set "VERSION=@VERSION@"
set "ARCH=x86_64"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ARCH=arm64"
if not exist "%~dp0..\cli\cli\BUILD.bazel" goto pinned
findstr /b /c:"    name = \"rules_dx\"," "%~dp0..\MODULE.bazel" >nul 2>&1
if errorlevel 1 goto pinned
if exist "%~dp0..\bazel-bin\cli\cli\dx.exe" goto run
pushd "%~dp0.."
call bazel build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx 1>&2
if errorlevel 1 (
  popd
  echo dx: bazel build //:dx failed 1>&2
  exit /b 1
)
popd
:run
"%~dp0..\bazel-bin\cli\cli\dx.exe" %*
exit /b %errorlevel%
:pinned
set "PIN="
set /p PIN=<"%~dp0..\.dx\version"
if not "%PIN%"=="%VERSION%" (
  echo dx: tools\dx.cmd is pinned to dx %VERSION% but .dx\version is '%PIN%'; run dx upgrade 1>&2
  goto bazel
)
set "DIR=%USERPROFILE%\.cache\dx\bin\%VERSION%"
set "BIN=%DIR%\dx-windows-%ARCH%.exe"
if exist "%BIN%" goto run
set "DX_URL=https://github.com/devhusdk/rules_dx/releases/download/v%VERSION%/dx-windows-%ARCH%"
set "DX_PART=%DIR%\.dx-%ARCH%.%RANDOM%.part"
mkdir "%DIR%" 2>nul
call powershell -NoProfile -NonInteractive -Command "$ErrorActionPreference='Stop'; Invoke-WebRequest -UseBasicParsing -Uri $env:DX_URL -OutFile $env:DX_PART; Invoke-WebRequest -UseBasicParsing -Uri ($env:DX_URL + '.sha256') -OutFile ($env:DX_PART + '.sha')"
if errorlevel 1 (
  call :clean
  echo dx: download failed: %DX_URL% 1>&2
  goto bazel
)
set "WANT="
set /p WANT=<"%DX_PART%.sha"
for /f "tokens=1" %%W in ("%WANT%") do set "WANT=%%W"
set "GOT="
for /f "skip=1 tokens=*" %%H in ('certutil -hashfile "%DX_PART%" SHA256') do if not defined GOT set "GOT=%%H"
set "GOT=%GOT: =%"
if "%WANT%"=="" goto mismatch
if /i not "%GOT%"=="%WANT%" goto mismatch
move /y "%DX_PART%" "%BIN%" >nul
call :clean
:run
"%BIN%" %*
exit /b %errorlevel%
:mismatch
call :clean
echo dx: sha256 mismatch for %DX_URL%: want '%WANT%', got '%GOT%'; refusing to run it 1>&2
goto bazel
:clean
del /q "%DX_PART%" "%DX_PART%.sha" 2>nul
exit /b 0
:bazel
echo dx: run it through Bazel instead: bazel run @rules_dx//:dx -- %* 1>&2
exit /b 1
"##;

pub fn render_launcher_sh(version: &str) -> String {
    LAUNCHER_SH.replace("@VERSION@", version)
}

pub fn render_launcher_cmd(version: &str) -> String {
    LAUNCHER_CMD
        .replace("@VERSION@", version)
        .replace('\n', "\r\n")
}

fn write_launcher(
    root: &Path,
    rel: &str,
    content: &str,
    executable: bool,
) -> Result<(), AdoptError> {
    let dest = super::scaffold_dest_within_root(root, rel)?;
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

pub fn write_launcher_files(
    root: &Path,
    version: &str,
    overwrite: bool,
) -> Result<Vec<String>, AdoptError> {
    let sh = render_launcher_sh(version);
    let cmd = render_launcher_cmd(version);
    let mut entries = Vec::new();
    for (rel, content, executable) in [(LAUNCHER_SH_REL, sh, true), (LAUNCHER_CMD_REL, cmd, false)]
    {
        let dest = super::scaffold_dest_within_root(root, rel)?;
        if dest.exists() && !overwrite {
            entries.push(format!("refused:{rel}"));
            continue;
        }
        write_launcher(root, rel, &content, executable)?;
        entries.push(rel.to_owned());
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_builds_dx_from_a_rules_dx_checkout() {
        let sh = render_launcher_sh("1.2.3");
        assert!(sh.contains("bazel build --noshow_progress --show_result=0"));
        assert!(sh.contains("MINGW* | MSYS* | CYGWIN*"));
        let cmd = render_launcher_cmd("1.2.3");
        assert!(cmd.contains("call bazel build --noshow_progress --show_result=0"));
        assert!(cmd.contains("\\bazel-bin\\cli\\cli\\dx.exe"));
    }

    #[test]
    fn launcher_pins_the_version_and_reads_the_sidecar() {
        let sh = render_launcher_sh("1.2.3");
        assert!(sh.starts_with("#!/bin/sh\n"));
        assert!(sh.contains("VERSION=\"1.2.3\""));
        assert!(sh.contains("fetch \"$url.sha256\""));
        assert!(!sh.contains("@VERSION@"));
        let cmd = render_launcher_cmd("1.2.3");
        assert!(cmd.contains("set \"VERSION=1.2.3\"\r\n"));
        assert!(cmd.contains("DX_URL + '.sha256'"));
        assert!(!cmd.contains("@VERSION@"));
        assert!(!cmd.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn write_is_absent_only_unless_overwriting() {
        let dir = dx_test_scratch::scratch("dx-launcher-write-");
        let first = write_launcher_files(dir.path(), "1.2.3", false).expect("writes");
        assert_eq!(first, vec!["tools/dx", "tools/dx.cmd"]);
        std::fs::write(dir.path().join(LAUNCHER_SH_REL), "custom\n").expect("edit");
        let second = write_launcher_files(dir.path(), "1.2.3", false).expect("keeps");
        assert_eq!(second, vec!["refused:tools/dx", "refused:tools/dx.cmd"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(LAUNCHER_SH_REL)).expect("read"),
            "custom\n"
        );
        let third = write_launcher_files(dir.path(), "1.3.0", true).expect("overwrites");
        assert_eq!(third, vec!["tools/dx", "tools/dx.cmd"]);
        let sh = std::fs::read_to_string(dir.path().join(LAUNCHER_SH_REL)).expect("read");
        assert!(sh.contains("VERSION=\"1.3.0\""));
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

    #[cfg(unix)]
    mod run {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        const PAYLOAD: &str = "#!/bin/sh\necho ran \"$@\"\n";

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
                let payload = dir.path().join("payload");
                std::fs::write(&payload, PAYLOAD).expect("payload");
                let sha = pinned_sha.map_or_else(|| sha_of(&payload), str::to_owned);
                std::fs::write(dir.path().join("sidecar"), format!("{sha}  dx\n")).expect("sha");
                let curl = dir.path().join("stub/curl");
                let body = if curl_ok {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; fi\n  url=\"$1\"\n  shift\ndone\ncase \"$url\" in\n  *.sha256) cp \"$STUB_SIDECAR\" \"$out\" ;;\n  *) cp \"$STUB_PAYLOAD\" \"$out\" ;;\nesac\n"
                } else {
                    "#!/bin/sh\necho call >> \"$STUB_LOG\"\nexit 6\n"
                };
                std::fs::write(&curl, body).expect("curl");
                std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
                write_launcher_files(&root, "1.2.3", false).expect("launcher");
                Self {
                    base: dir.path().to_path_buf(),
                    _guard: Box::new(dir),
                }
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
                let base = &self.base;
                let out = Command::new(base.join("repo/tools/dx"))
                    .args(args)
                    .env_clear()
                    .env(
                        "PATH",
                        format!("{}:/usr/bin:/bin", base.join("stub").display()),
                    )
                    .env("HOME", base.join("home"))
                    .env("STUB_LOG", base.join("calls"))
                    .env("STUB_PAYLOAD", base.join("payload"))
                    .env("STUB_SIDECAR", base.join("sidecar"))
                    .output()
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
            assert_eq!(fx.downloads(), 2);
            assert_eq!(fx.cached().len(), 1, "{:?}", fx.cached());
            let (code, out, err) = fx.run(&["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(fx.downloads(), 2, "cached run must not download");
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

        #[test]
        fn rosetta_on_apple_silicon_runs_the_arm64_binary() {
            let fx = Fixture::new("dx-launcher-rosetta-", true, None);
            fx.stub("uname", UNAME_ROSETTA);
            fx.stub("sysctl", "#!/bin/sh\necho 1\n");
            let (code, out, err) = fx.run(&["version"]);
            assert_eq!((code, out.as_str()), (Some(0), "ran version\n"), "{err}");
            assert_eq!(fx.cached(), vec!["dx-macos-arm64"]);
        }

        #[test]
        fn intel_mac_still_has_no_binary() {
            let fx = Fixture::new("dx-launcher-intel-mac-", true, None);
            fx.stub("uname", UNAME_ROSETTA);
            fx.stub("sysctl", "#!/bin/sh\nexit 1\n");
            let (code, out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1), "{out}{err}");
            assert!(err.contains("no dx binary for macos x86_64"), "{err}");
            assert_eq!(fx.downloads(), 0);
            assert!(fx.cached().is_empty(), "{:?}", fx.cached());
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
                let system = std::env::var("SystemRoot").expect("SystemRoot");
                let payload = base.join("payload");
                std::fs::copy(format!("{system}\\System32\\cmd.exe"), &payload).expect("payload");
                let want = sha.map_or_else(|| sha_of(&payload), str::to_owned);
                std::fs::write(base.join("sidecar"), format!("{want}  dx\r\n")).expect("sha");
                let body = if ps_ok {
                    "@echo off\r\necho call>>\"%STUB_LOG%\"\r\ncopy /y \"%STUB_PAYLOAD%\" \"%DX_PART%\" >nul\r\ncopy /y \"%STUB_SIDECAR%\" \"%DX_PART%.sha\" >nul\r\nexit /b 0\r\n"
                } else {
                    "@echo off\r\necho call>>\"%STUB_LOG%\"\r\necho partial>\"%DX_PART%\"\r\nexit /b 9009\r\n"
                };
                std::fs::write(base.join("stub/powershell.cmd"), body).expect("stub");
                write_launcher_files(&root, "1.2.3", false).expect("launcher");
                Self {
                    base,
                    _guard: Box::new(dir),
                }
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
                let base = &self.base;
                let system = std::env::var("SystemRoot").expect("SystemRoot");
                let out = Command::new("cmd")
                    .arg("/c")
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
                    .output()
                    .expect("launcher runs");
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                    String::from_utf8_lossy(&out.stderr).into_owned(),
                )
            }

            fn downloads(&self) -> usize {
                std::fs::read_to_string(self.base.join("calls"))
                    .map_or(0, |text| text.lines().count())
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
        fn a_pin_that_differs_from_the_launcher_is_refused() {
            let fx = Fixture::new("dx-launcher-cmd-pin-", true, None);
            std::fs::write(fx.base.join("repo/.dx/version"), "2.0.0\r\n").expect("pin");
            let (code, _out, err) = fx.run(&["version"]);
            assert_eq!(code, Some(1));
            assert!(
                err.contains("pinned to dx 1.2.3 but .dx\\version is '2.0.0'"),
                "{err}"
            );
            assert_eq!(fx.downloads(), 0);
        }
    }
}
