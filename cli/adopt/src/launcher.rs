use std::path::Path;

use super::AdoptError;

pub const LAUNCHER_SH_REL: &str = "tools/dx";

pub const LAUNCHER_CMD_REL: &str = "tools/dx.cmd";

const LAUNCHER_SH: &str = r##"#!/bin/sh
set -eu
VERSION="@VERSION@"
args="$*"
die() {
  echo "dx: $1" >&2
  exit 1
}
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
source_checkout() {
  [ -f "$1/cli/cli/BUILD.bazel" ] && grep -q '^    name = "rules_dx",' "$1/MODULE.bazel" 2> /dev/null
}
if source_checkout "$root"; then
  bin="$root/bazel-bin/cli/cli/dx$exe"
  state="$root/.dx/launcher"
  if [ "${1:-}" = worktree ] && [ "${2:-}" = hook ] && [ -x "$bin" ]; then
    exec "$bin" "$@"
  fi
  here=$(pwd -P) || here=/
  until [ "$here" = / ] || source_checkout "$here"; do
    here=${here%/*}
    here=${here:-/}
  done
  if [ "$here" != / ] && [ "$here" != "$(cd -P -- "$root" && pwd -P)" ] && [ -x "$here/tools/dx" ]; then
    exec "$here/tools/dx" "$@"
  fi
  stale() {
    [ -x "$bin" ] && [ -s "$state/inputs" ] && [ -f "$state/built" ] || return 0
    newer=$(cd "$root" && {
      sed -e 's|^//||' -e 's|^:||' -e 's|:|/|' "$state/inputs"
      printf '%s\n' .bazelrc .bazelversion user.bazelrc tools/bazelrc/preset.bazelrc MODULE.bazel MODULE.bazel.lock cargo-bazel-lock.json
    } | tr '\n' '\0' | xargs -0 sh -c 'find "$@" -newer "$0" -print -quit' "$state/built" 2> /dev/null) || true
    [ -n "$newer" ]
  }
  keep_copy() {
    { cp "$bin" "$state/dx$exe.$$" && mv -f "$state/dx$exe.$$" "$state/dx$exe"; } 2> /dev/null || rm -f "$state/dx$exe.$$"
  }
  if stale; then
    command -v bazel > /dev/null 2>&1 || die "bazel is not on PATH"
    mkdir -p "$state"
    : > "$state/built.$$"
    trap 'rm -f "$state/built.$$" "$state/inputs.$$" "$state/dx$exe.$$"' EXIT
    rc="--nohome_rc --nosystem_rc"
    [ "${CI:-}" = true ] || [ -n "${BAZEL_TEST:-}" ] || rc=
    echo "dx: building //:dx" >&2
    if ! (cd "$root" && bazel $rc build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx >&2); then
      if [ -x "$state/dx$exe" ]; then
        rm -f "$state/built.$$"
        echo "dx: //:dx does not build; running the previous dx" >&2
        exec "$state/dx$exe" "$@"
      fi
      die "bazel build //:dx failed"
    fi
    keep_copy
    q=$(cd "$root" && bazel $rc query --noshow_progress --ui_event_filters=-info 'kind("source file", deps(//cli/cli:dx)) + buildfiles(deps(//cli/cli:dx))') || die "bazel query for the inputs of //cli/cli:dx failed"
    printf '%s\n' "$q" | tr -d '\r' | grep '^//' > "$state/inputs.$$" || true
    mv "$state/inputs.$$" "$state/inputs"
    mv "$state/built.$$" "$state/built"
  elif [ ! -x "$state/dx$exe" ] && [ -x "$bin" ]; then
    trap 'rm -f "$state/dx$exe.$$"' EXIT
    keep_copy
  fi
  exec "$bin" "$@"
fi
IFS= read -r pin < "$root/.dx/version" || true
pin=$(printf %s "$pin" | tr -d '\r ')
[ "$pin" = "$VERSION" ] || fail "tools/dx is pinned to dx $VERSION but .dx/version is '$pin'; run dx upgrade"
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
  linux-x86_64 | linux-arm64 | macos-arm64 | windows-x86_64 | windows-arm64) ;;
  *) fail "no dx binary for $os $arch" ;;
esac
if [ "$os" = windows ]; then
  [ -n "${USERPROFILE:-}" ] || fail "USERPROFILE is not set"
  dir="$(cygpath -u "$USERPROFILE")/.cache/dx/bin/$VERSION"
else
  [ -n "${HOME:-}" ] || fail "HOME is not set"
  dir="$HOME/.cache/dx/bin/$VERSION"
fi
bin="$dir/dx-$os-$arch$exe"
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
set "BIN=%~dp0..\bazel-bin\cli\cli\dx.exe"
set "DX_ROOT=%~dp0.."
set "DX_STATE=%~dp0..\.dx\launcher"
if /i "%~1"=="worktree" if /i "%~2"=="hook" if exist "%BIN%" goto run
for %%P in ("%DX_ROOT%") do set "DX_SELF=%%~fP"
set "DX_HERE=%CD%"
:dxup
if not exist "%DX_HERE%\cli\cli\BUILD.bazel" goto dxnext
findstr /b /c:"    name = \"rules_dx\"," "%DX_HERE%\MODULE.bazel" >nul 2>&1
if not errorlevel 1 goto dxfound
:dxnext
set "DX_UP="
for %%P in ("%DX_HERE%\..") do set "DX_UP=%%~fP"
if /i "%DX_UP%"=="%DX_HERE%" goto dxself
set "DX_HERE=%DX_UP%"
goto dxup
:dxfound
if /i "%DX_HERE%"=="%DX_SELF%" goto dxself
if not exist "%DX_HERE%\tools\dx.cmd" goto dxself
"%DX_HERE%\tools\dx.cmd" %*
:dxself
call powershell -NoProfile -NonInteractive -Command "$s=$env:DX_STATE; $b=Join-Path $s 'built'; $i=Join-Path $s 'inputs'; if (-not ([IO.File]::Exists($env:BIN) -and [IO.File]::Exists($b) -and [IO.File]::Exists($i))) { exit 1 }; if ((Get-Item -LiteralPath $i).Length -eq 0) { exit 1 }; $t=[IO.File]::GetLastWriteTimeUtc($b); $p=@(Get-Content -LiteralPath $i | ForEach-Object { $_.TrimStart('/').TrimStart(':').Replace(':', '/') }) + @('.bazelrc', '.bazelversion', 'user.bazelrc', 'tools/bazelrc/preset.bazelrc', 'MODULE.bazel', 'MODULE.bazel.lock', 'cargo-bazel-lock.json'); foreach ($f in $p) { if ([IO.File]::GetLastWriteTimeUtc((Join-Path $env:DX_ROOT $f)) -gt $t) { exit 1 } }; $c=Join-Path $s 'dx.exe'; if (-not [IO.File]::Exists($c)) { $n=$c + '.' + [guid]::NewGuid(); try { [IO.File]::Copy($env:BIN, $n); [IO.File]::Move($n, $c) } catch { Remove-Item -LiteralPath $n -Force -ErrorAction SilentlyContinue } }; exit 0"
if not errorlevel 1 goto run
if not exist "%DX_STATE%" mkdir "%DX_STATE%"
set "DX_TAG="
for /f %%G in ('powershell -NoProfile -NonInteractive -Command "[guid]::NewGuid().ToString()"') do set "DX_TAG=%%G"
if not defined DX_TAG (
  echo dx: powershell is needed to rebuild dx 1>&2
  exit /b 1
)
type nul > "%DX_STATE%\built.%DX_TAG%"
pushd "%DX_ROOT%"
set "DX_RC=--nohome_rc --nosystem_rc"
if not "%CI%"=="true" if "%BAZEL_TEST%"=="" set "DX_RC="
echo dx: building //:dx 1>&2
call bazel %DX_RC% build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx 1>&2
if errorlevel 1 (
  popd
  if exist "%DX_STATE%\dx.exe" (
    echo dx: //:dx does not build; running the previous dx 1>&2
    del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
    set "BIN=%DX_STATE%\dx.exe"
    goto run
  )
  echo dx: bazel build //:dx failed 1>&2
  del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
  exit /b 1
)
copy /y "%BIN%" "%DX_STATE%\dx.exe.%DX_TAG%" >nul 2>&1
move /y "%DX_STATE%\dx.exe.%DX_TAG%" "%DX_STATE%\dx.exe" >nul 2>&1
call bazel %DX_RC% query --noshow_progress --ui_event_filters=-info "kind('source file', deps(//cli/cli:dx)) + buildfiles(deps(//cli/cli:dx))" > "%DX_STATE%\query.%DX_TAG%"
if errorlevel 1 (
  popd
  echo dx: bazel query for the inputs of //cli/cli:dx failed 1>&2
  del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
  exit /b 1
)
popd
findstr /b /c:"//" "%DX_STATE%\query.%DX_TAG%" > "%DX_STATE%\inputs.%DX_TAG%"
del /q "%DX_STATE%\query.%DX_TAG%"
move /y "%DX_STATE%\inputs.%DX_TAG%" "%DX_STATE%\inputs" >nul
move /y "%DX_STATE%\built.%DX_TAG%" "%DX_STATE%\built" >nul
del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
:run
"%BIN%" %*
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
        let startup = dx_process::WORKFLOW_STARTUP_OPTS.join(" ");
        let sh = render_launcher_sh("1.2.3");
        assert!(sh.contains(&format!("    rc=\"{startup}\"\n")));
        assert!(sh.contains("    [ \"${CI:-}\" = true ] || [ -n \"${BAZEL_TEST:-}\" ] || rc=\n"));
        assert!(sh.contains("(cd \"$root\" && bazel $rc build --noshow_progress --show_result=0"));
        assert!(sh.contains("(cd \"$root\" && bazel $rc query --noshow_progress"));
        assert!(sh.contains("$root/.dx/launcher"));
        assert!(sh.contains("\"$state/inputs\""));
        assert!(!sh.contains("cmd.exe"));
        let hook = sh
            .find("  if [ \"${1:-}\" = worktree ] && [ \"${2:-}\" = hook ] && [ -x \"$bin\" ]; then\n    exec \"$bin\" \"$@\"\n  fi\n")
            .expect("sh hook early exec");
        let handoff = sh
            .find("\n    exec \"$here/tools/dx\" \"$@\"\n")
            .expect("sh hand-off to the current checkout");
        assert!(hook < handoff);
        assert!(handoff < sh.find("if stale; then").expect("stale check"));
        let cmd = render_launcher_cmd("1.2.3");
        let hook = cmd
            .find("\r\nif /i \"%~1\"==\"worktree\" if /i \"%~2\"==\"hook\" if exist \"%BIN%\" goto run\r\n")
            .expect("cmd hook early exec");
        let handoff = cmd
            .find("\r\n\"%DX_HERE%\\tools\\dx.cmd\" %*\r\n:dxself\r\n")
            .expect("cmd hand-off to the current checkout");
        assert!(hook < handoff);
        assert!(
            handoff
                < cmd
                    .find("call powershell -NoProfile -NonInteractive -Command \"$s=")
                    .expect("stale check")
        );
        assert!(cmd.contains(&format!("\r\nset \"DX_RC={startup}\"\r\n")));
        assert!(cmd.contains(
            "\r\nif not \"%CI%\"==\"true\" if \"%BAZEL_TEST%\"==\"\" set \"DX_RC=\"\r\n"
        ));
        assert!(cmd.contains("\ncall bazel %DX_RC% build --noshow_progress --show_result=0"));
        assert!(cmd.contains("\ncall bazel %DX_RC% query --noshow_progress"));
        assert!(cmd.contains("\\.dx\\launcher"));
        assert!(cmd.contains("\\bazel-bin\\cli\\cli\\dx.exe"));
        assert!(!cmd.contains("if exist \"%~dp0..\\bazel-bin"));
        assert!(sh.contains("exec \"$state/dx$exe\" \"$@\""));
        assert!(cmd.contains("set \"BIN=%DX_STATE%\\dx.exe\""));
        assert!(sh.contains(
            "\n  elif [ ! -x \"$state/dx$exe\" ] && [ -x \"$bin\" ]; then\n    trap 'rm -f \"$state/dx$exe.$$\"' EXIT\n    keep_copy\n  fi\n"
        ));
        assert_eq!(sh.matches("\n    keep_copy\n").count(), 2);
        assert!(cmd.contains("[IO.File]::Copy($env:BIN, $n); [IO.File]::Move($n, $c)"));
        for script in [&sh, &cmd] {
            assert!(script.contains("dx: building //:dx"));
            assert!(script.contains("dx: //:dx does not build; running the previous dx"));
        }
        let dev = sh
            .find("if source_checkout \"$root\"; then")
            .expect("dev sh");
        let pinned = sh.find("IFS= read -r pin").expect("pinned sh");
        let dev_sh = &sh[dev..pinned];
        assert!(!dev_sh.contains("fail "), "{dev_sh}");
        let dev_cmd = &cmd[..cmd.find("\r\n:pinned\r\n").expect("pinned cmd")];
        assert!(!dev_cmd.contains("bazel run @rules_dx"));
        assert!(!dev_cmd.contains("goto bazel"));
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
                let out = spawn(
                    Command::new(base.join("repo/tools/dx"))
                        .args(args)
                        .env_clear()
                        .env(
                            "PATH",
                            format!("{}:/usr/bin:/bin", base.join("stub").display()),
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

        struct SourceRepo {
            _guard: Box<dyn std::any::Any>,
            base: std::path::PathBuf,
        }

        fn checkout(root: &Path) {
            std::fs::create_dir_all(root.join("cli/cli")).expect("cli dir");
            std::fs::create_dir_all(root.join("tools/bazelrc")).expect("bazelrc dir");
            std::fs::write(
                root.join("MODULE.bazel"),
                "module(\n    name = \"rules_dx\",\n)\n",
            )
            .expect("module");
            std::fs::write(root.join("cli/cli/BUILD.bazel"), "").expect("build file");
            std::fs::write(root.join("tools/bazelrc/preset.bazelrc"), "").expect("preset");
            write_launcher_files(root, "1.2.3", false).expect("launcher");
        }

        fn source_repo(name: &str) -> SourceRepo {
            let dir = dx_test_scratch::scratch(name);
            let base = dir.path().to_path_buf();
            let root = base.join("repo");
            checkout(&root);
            std::fs::create_dir_all(base.join("stub")).expect("stub dir");
            std::fs::create_dir_all(base.join("home")).expect("home");
            std::fs::write(base.join("payload"), PAYLOAD).expect("payload");
            let stub = base.join("stub/bazel");
            std::fs::write(
                &stub,
                "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in\n    build)\n      echo build >> \"$STUB_LOG\"\n      sleep 1\n      mkdir -p bazel-bin/cli/cli\n      src=\"$STUB_PAYLOAD\"\n      [ -f payload ] && src=payload\n      cp \"$src\" \"bazel-bin/cli/cli/dx.$$\"\n      chmod +x \"bazel-bin/cli/cli/dx.$$\"\n      mv \"bazel-bin/cli/cli/dx.$$\" bazel-bin/cli/cli/dx\n      exit 0\n      ;;\n    query)\n      echo //cli/cli:BUILD.bazel\n      sleep 1\n      echo @rules_rust//rust:defs.bzl\n      exit 0\n      ;;\n  esac\ndone\nexit 2\n",
            )
            .expect("bazel stub");
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
            SourceRepo {
                base,
                _guard: Box::new(dir),
            }
        }

        impl SourceRepo {
            fn root(&self) -> std::path::PathBuf {
                self.base.join("repo")
            }

            fn command(&self) -> Command {
                self.command_in(&self.root(), &["status"])
            }

            fn command_in(&self, cwd: &Path, args: &[&str]) -> Command {
                let base = &self.base;
                let mut cmd = Command::new(base.join("repo/tools/dx"));
                cmd.args(args)
                    .current_dir(cwd)
                    .env_clear()
                    .env(
                        "PATH",
                        format!("{}:/usr/bin:/bin", base.join("stub").display()),
                    )
                    .env("HOME", base.join("home"))
                    .env("STUB_LOG", base.join("calls"))
                    .env("STUB_PAYLOAD", base.join("payload"));
                cmd
            }

            fn spawn(&self) -> std::process::Child {
                spawn(&mut self.command())
            }

            fn run_ok(&self) {
                let out = self.spawn().wait_with_output().expect("launcher runs");
                assert_eq!(
                    (
                        out.status.code(),
                        String::from_utf8_lossy(&out.stdout).into_owned()
                    ),
                    (Some(0), "ran status\n".to_owned()),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }

            fn run_in(&self, cwd: &Path, args: &[&str]) -> (Option<i32>, String, String) {
                let out = spawn(&mut self.command_in(cwd, args))
                    .wait_with_output()
                    .expect("launcher runs");
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                    String::from_utf8_lossy(&out.stderr).into_owned(),
                )
            }

            fn builds(&self) -> usize {
                std::fs::read_to_string(self.base.join("calls"))
                    .map_or(0, |text| text.lines().filter(|l| *l == "build").count())
            }

            fn inputs(&self) -> String {
                std::fs::read_to_string(self.root().join(".dx/launcher/inputs")).expect("inputs")
            }

            fn state_files(&self) -> Vec<String> {
                let mut names: Vec<String> = std::fs::read_dir(self.root().join(".dx/launcher"))
                    .expect("state dir")
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                names
            }

            fn touch_build_file(&self) {
                let file = std::fs::File::options()
                    .write(true)
                    .open(self.root().join("cli/cli/BUILD.bazel"))
                    .expect("open build file");
                file.set_modified(
                    std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
                )
                .expect("touch build file");
            }

            fn break_build(&self) {
                let stub = self.base.join("stub/bazel");
                std::fs::write(
                    &stub,
                    "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in\n    build)\n      echo build >> \"$STUB_LOG\"\n      rm -f bazel-bin/cli/cli/dx\n      exit 1\n      ;;\n  esac\ndone\nexit 2\n",
                )
                .expect("failing bazel stub");
                std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
            }

            fn run(&self) -> (Option<i32>, String, String) {
                let out = self.spawn().wait_with_output().expect("launcher runs");
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                    String::from_utf8_lossy(&out.stderr).into_owned(),
                )
            }
        }

        #[test]
        fn source_launcher_runs_the_previous_dx_when_dx_does_not_build() {
            let repo = source_repo("dx-launcher-fallback-");
            repo.run_ok();
            assert!(repo.root().join(".dx/launcher/dx").exists());
            repo.break_build();
            repo.touch_build_file();
            let (code, out, err) = repo.run();
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert!(err.contains("dx: building //:dx\n"), "{err}");
            assert!(
                err.contains("dx: //:dx does not build; running the previous dx\n"),
                "{err}"
            );
            assert!(!err.contains("bazel run @rules_dx"), "{err}");
            assert!(!repo.root().join("bazel-bin/cli/cli/dx").exists());
            assert_eq!(repo.builds(), 2);
            assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
            let (code, out, err) = repo.run();
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert_eq!(repo.builds(), 3, "a failed build is tried again");
        }

        #[test]
        fn source_launcher_seeds_the_previous_dx_when_dx_is_up_to_date() {
            let repo = source_repo("dx-launcher-seed-");
            repo.run_ok();
            std::fs::remove_file(repo.root().join(".dx/launcher/dx")).expect("remove the copy");
            repo.run_ok();
            assert_eq!(repo.builds(), 1, "seeding the copy does not rebuild");
            assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
            repo.break_build();
            repo.touch_build_file();
            let (code, out, err) = repo.run();
            assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert!(
                err.contains("dx: //:dx does not build; running the previous dx\n"),
                "{err}"
            );
            assert_eq!(repo.builds(), 2);
        }

        #[test]
        fn a_worktree_hook_run_does_not_seed_the_previous_dx() {
            let repo = source_repo("dx-launcher-hook-seed-");
            repo.run_ok();
            std::fs::remove_file(repo.root().join(".dx/launcher/dx")).expect("remove the copy");
            let (code, out, err) = repo.run_in(&repo.root(), &["worktree", "hook", "guard"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran worktree hook guard\n"),
                "{err}"
            );
            assert_eq!(repo.state_files(), vec!["built", "inputs"]);
            assert_eq!(repo.builds(), 1);
        }

        #[test]
        fn source_launcher_without_a_previous_dx_reports_the_failed_build() {
            let repo = source_repo("dx-launcher-no-fallback-");
            repo.break_build();
            let (code, out, err) = repo.run();
            assert_eq!((code, out.as_str()), (Some(1), ""), "{err}");
            assert!(err.contains("dx: building //:dx\n"), "{err}");
            assert!(err.contains("dx: bazel build //:dx failed\n"), "{err}");
            assert!(!err.contains("bazel run @rules_dx"), "{err}");
            assert!(!err.contains("previous dx"), "{err}");
            assert_eq!(repo.state_files(), Vec::<String>::new());
        }

        #[test]
        fn source_launcher_rebuilds_only_when_an_input_is_newer() {
            let repo = source_repo("dx-launcher-source-");
            repo.run_ok();
            assert_eq!(repo.builds(), 1, "a missing dx is built");
            assert_eq!(repo.inputs(), "//cli/cli:BUILD.bazel\n");
            repo.run_ok();
            assert_eq!(repo.builds(), 1, "an unchanged tree runs dx without Bazel");
            repo.touch_build_file();
            repo.run_ok();
            assert_eq!(repo.builds(), 2, "a newer input rebuilds dx");
        }

        #[test]
        fn a_launcher_run_from_another_checkout_runs_that_checkout_dx() {
            let repo = source_repo("dx-launcher-slot-");
            let slot = repo.base.join("slot");
            checkout(&slot);
            std::fs::write(slot.join("payload"), "#!/bin/sh\necho slot \"$@\"\n")
                .expect("slot payload");
            let (code, out, err) = repo.run_in(&slot.join("cli"), &["status"]);
            assert_eq!((code, out.as_str()), (Some(0), "slot status\n"), "{err}");
            assert!(slot.join(".dx/launcher/built").exists());
            assert!(!repo.root().join(".dx/launcher").exists());
            assert_eq!(repo.builds(), 1, "only the slot dx is built");
            repo.run_ok();
            assert_eq!(repo.builds(), 2, "the repo builds its own dx");
            let (code, out, err) = repo.run_in(&slot, &["worktree", "hook", "guard"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran worktree hook guard\n"),
                "{err}"
            );
            assert_eq!(repo.builds(), 2, "a hook never builds the slot dx");
            repo.run_ok();
            assert_eq!(repo.builds(), 2, "the repo runs its dx in place");
        }

        #[test]
        fn concurrent_stale_runs_all_succeed_and_keep_the_inputs() {
            let repo = source_repo("dx-launcher-concurrent-");
            let children: Vec<_> = (0..4).map(|_| repo.spawn()).collect();
            for child in children {
                let out = child.wait_with_output().expect("launcher runs");
                assert_eq!(
                    (
                        out.status.code(),
                        String::from_utf8_lossy(&out.stdout).into_owned()
                    ),
                    (Some(0), "ran status\n".to_owned()),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
            assert!(
                repo.inputs().contains("//cli/cli:BUILD.bazel"),
                "{}",
                repo.inputs()
            );
            let before = repo.builds();
            repo.touch_build_file();
            repo.run_ok();
            assert_eq!(repo.builds(), before + 1, "a newer input still rebuilds dx");
        }

        #[test]
        fn an_empty_input_list_rebuilds() {
            let repo = source_repo("dx-launcher-empty-inputs-");
            repo.run_ok();
            assert_eq!(repo.builds(), 1);
            std::fs::write(repo.root().join(".dx/launcher/inputs"), "").expect("truncate");
            repo.run_ok();
            assert_eq!(repo.builds(), 2, "an empty input list rebuilds dx");
            assert_eq!(repo.inputs(), "//cli/cli:BUILD.bazel\n");
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
            assert_eq!(fx.downloads(), 2);
            assert_eq!(fx.cached(), vec!["dx-windows-x86_64.exe"]);
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

        struct SourceRepo {
            _guard: Box<dyn std::any::Any>,
            base: std::path::PathBuf,
        }

        fn checkout(root: &Path) {
            std::fs::create_dir_all(root.join("cli/cli")).expect("cli dir");
            std::fs::create_dir_all(root.join("tools/bazelrc")).expect("bazelrc dir");
            std::fs::write(
                root.join("MODULE.bazel"),
                "module(\r\n    name = \"rules_dx\",\r\n)\r\n",
            )
            .expect("module");
            std::fs::write(root.join("cli/cli/BUILD.bazel"), "").expect("build file");
            std::fs::write(root.join("tools/bazelrc/preset.bazelrc"), "").expect("preset");
            write_launcher_files(root, "1.2.3", false).expect("launcher");
        }

        fn source_repo(name: &str) -> SourceRepo {
            let dir = dx_test_scratch::scratch(name);
            let base = dir.path().to_path_buf();
            checkout(&base.join("repo"));
            std::fs::create_dir_all(base.join("stub")).expect("stub dir");
            std::fs::write(
                base.join("stub/bazel.cmd"),
                "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\nif \"%1\"==\"query\" set \"OP=query\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  if not exist bazel-bin\\cli\\cli mkdir bazel-bin\\cli\\cli\r\n  copy /y \"%SystemRoot%\\System32\\hostname.exe\" bazel-bin\\cli\\cli\\dx.exe >nul\r\n)\r\nif \"%OP%\"==\"query\" (\r\n  echo //cli/cli:BUILD.bazel\r\n  ping -n 2 127.0.0.1 >nul\r\n  echo @rules_rust//rust:defs.bzl\r\n)\r\nexit /b 0\r\n",
            )
            .expect("bazel stub");
            SourceRepo {
                base,
                _guard: Box::new(dir),
            }
        }

        impl SourceRepo {
            fn root(&self) -> std::path::PathBuf {
                self.base.join("repo")
            }

            fn command(&self, args: &[&str], log: &Path) -> Command {
                self.command_in(&self.root(), args, log)
            }

            fn command_in(&self, cwd: &Path, args: &[&str], log: &Path) -> Command {
                let system = std::env::var("SystemRoot").expect("SystemRoot");
                let mut cmd = Command::new("cmd");
                cmd.arg("/c")
                    .arg(self.root().join("tools/dx.cmd"))
                    .args(args)
                    .current_dir(cwd)
                    .env(
                        "PATH",
                        format!(
                            "{};{system}\\System32;{system}\\System32\\WindowsPowerShell\\v1.0",
                            self.base.join("stub").display()
                        ),
                    )
                    .env("STUB_LOG", log);
                cmd
            }

            fn run(&self, args: &[&str]) -> (Option<i32>, String) {
                self.run_in(&self.root(), args)
            }

            fn run_in(&self, cwd: &Path, args: &[&str]) -> (Option<i32>, String) {
                let out = self
                    .command_in(cwd, args, &self.base.join("calls"))
                    .output()
                    .expect("launcher runs");
                (
                    out.status.code(),
                    format!(
                        "{}{}",
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    ),
                )
            }

            fn builds(&self) -> usize {
                std::fs::read_to_string(self.base.join("calls")).map_or(0, |text| {
                    text.lines().filter(|l| l.trim() == "build").count()
                })
            }

            fn state_files(&self) -> Vec<String> {
                let mut names: Vec<String> = std::fs::read_dir(self.root().join(".dx/launcher"))
                    .expect("state dir")
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                names
            }

            fn inputs(&self) -> String {
                std::fs::read_to_string(self.root().join(".dx/launcher/inputs")).expect("inputs")
            }
        }

        #[test]
        fn cmd_launcher_rebuilds_only_when_an_input_is_newer() {
            let repo = source_repo("dx-launcher-cmd-source-");
            let root = repo.root();
            let run = |args: &[&str]| repo.run(args);
            let run_ok = |args: &[&str]| {
                let (code, out) = run(args);
                assert_eq!(code, Some(0), "{out}");
            };
            let builds = || repo.builds();
            run_ok(&[]);
            assert_eq!(builds(), 1, "a missing dx is built");
            assert!(root.join(".dx/launcher/inputs").exists());
            run_ok(&[]);
            assert_eq!(builds(), 1, "an unchanged tree runs dx without Bazel");
            let preset = std::fs::File::options()
                .write(true)
                .open(root.join("tools/bazelrc/preset.bazelrc"))
                .expect("open preset");
            preset
                .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
                .expect("touch preset");
            drop(preset);
            run(&["worktree", "hook", "guard"]);
            assert_eq!(builds(), 1, "a hook runs the built dx without Bazel");
            run_ok(&[]);
            assert_eq!(builds(), 2, "a newer preset.bazelrc rebuilds dx");
        }

        #[test]
        fn cmd_a_launcher_run_from_another_checkout_runs_that_checkout_dx() {
            let repo = source_repo("dx-launcher-cmd-slot-");
            let slot = repo.base.join("slot");
            checkout(&slot);
            let (code, out) = repo.run_in(&slot.join("cli"), &[]);
            assert_eq!(code, Some(0), "{out}");
            assert!(slot.join(".dx/launcher/built").exists());
            assert!(slot.join("bazel-bin/cli/cli/dx.exe").exists());
            assert!(!repo.root().join(".dx/launcher").exists());
            assert_eq!(repo.builds(), 1, "only the slot dx is built");
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert!(repo.root().join(".dx/launcher/built").exists());
            assert_eq!(repo.builds(), 2, "the repo builds its own dx");
            std::fs::remove_file(slot.join("bazel-bin/cli/cli/dx.exe")).expect("drop slot dx");
            repo.run_in(&slot, &["worktree", "hook", "guard"]);
            assert_eq!(repo.builds(), 2, "a hook never builds the slot dx");
            assert!(!slot.join("bazel-bin/cli/cli/dx.exe").exists());
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert_eq!(repo.builds(), 2, "the repo runs its dx in place");
        }

        #[test]
        fn cmd_concurrent_stale_runs_all_succeed_and_keep_the_inputs() {
            let repo = source_repo("dx-launcher-cmd-concurrent-");
            let children: Vec<_> = (0..4)
                .map(|_| {
                    repo.command(&[], Path::new("NUL"))
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .expect("launcher starts")
                })
                .collect();
            for child in children {
                let out = child.wait_with_output().expect("launcher runs");
                assert_eq!(
                    out.status.code(),
                    Some(0),
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            assert_eq!(repo.state_files(), vec!["built", "dx.exe", "inputs"]);
            assert!(
                repo.inputs().contains("//cli/cli:BUILD.bazel"),
                "{}",
                repo.inputs()
            );
        }

        #[test]
        fn cmd_launcher_runs_the_previous_dx_when_dx_does_not_build() {
            let repo = source_repo("dx-launcher-cmd-fallback-");
            let root = repo.root();
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert!(out.contains("dx: building //:dx"), "{out}");
            assert!(root.join(".dx/launcher/dx.exe").exists());
            std::fs::write(
                repo.base.join("stub/bazel.cmd"),
                "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  del /q bazel-bin\\cli\\cli\\dx.exe\r\n  exit /b 1\r\n)\r\nexit /b 0\r\n",
            )
            .expect("failing bazel stub");
            let preset = std::fs::File::options()
                .write(true)
                .open(root.join("tools/bazelrc/preset.bazelrc"))
                .expect("open preset");
            preset
                .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
                .expect("touch preset");
            drop(preset);
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert!(out.contains("dx: building //:dx"), "{out}");
            assert!(
                out.contains("dx: //:dx does not build; running the previous dx"),
                "{out}"
            );
            assert!(!out.contains("bazel run @rules_dx"), "{out}");
            assert!(!root.join("bazel-bin/cli/cli/dx.exe").exists());
            assert_eq!(repo.builds(), 2);
            assert_eq!(repo.state_files(), vec!["built", "dx.exe", "inputs"]);
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert_eq!(repo.builds(), 3, "a failed build is tried again");
        }

        #[test]
        fn cmd_launcher_seeds_the_previous_dx_when_dx_is_up_to_date() {
            let repo = source_repo("dx-launcher-cmd-seed-");
            let root = repo.root();
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            std::fs::remove_file(root.join(".dx/launcher/dx.exe")).expect("remove the copy");
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert_eq!(repo.builds(), 1, "seeding the copy does not rebuild");
            assert_eq!(repo.state_files(), vec!["built", "dx.exe", "inputs"]);
            std::fs::write(
                repo.base.join("stub/bazel.cmd"),
                "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  del /q bazel-bin\\cli\\cli\\dx.exe\r\n  exit /b 1\r\n)\r\nexit /b 0\r\n",
            )
            .expect("failing bazel stub");
            let preset = std::fs::File::options()
                .write(true)
                .open(root.join("tools/bazelrc/preset.bazelrc"))
                .expect("open preset");
            preset
                .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
                .expect("touch preset");
            drop(preset);
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert!(
                out.contains("dx: //:dx does not build; running the previous dx"),
                "{out}"
            );
            assert_eq!(repo.builds(), 2);
        }

        #[test]
        fn cmd_an_empty_input_list_rebuilds() {
            let repo = source_repo("dx-launcher-cmd-empty-inputs-");
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert_eq!(repo.builds(), 1);
            std::fs::write(repo.root().join(".dx/launcher/inputs"), "").expect("truncate");
            let (code, out) = repo.run(&[]);
            assert_eq!(code, Some(0), "{out}");
            assert_eq!(repo.builds(), 2, "an empty input list rebuilds dx");
            assert!(
                repo.inputs().contains("//cli/cli:BUILD.bazel"),
                "{}",
                repo.inputs()
            );
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
