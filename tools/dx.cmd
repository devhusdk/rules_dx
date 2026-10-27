@echo off
setlocal
set "BIN=%~dp0..\bazel-bin\cli\cli\dx.exe"
set "DX_ROOT=%~dp0.."
set "DX_STATE=%~dp0..\.dx\launcher"
set "DX_COPY="
set "DX_SYNC=$c=Join-Path $env:DX_STATE 'dx.exe'; $o=$c + '.old.*'; function M { try { $x=[IO.FileInfo]::new($c); $y=[IO.FileInfo]::new($env:BIN); return ($x.Exists -and $y.Exists -and $x.Length -eq $y.Length -and $x.LastWriteTimeUtc -eq $y.LastWriteTimeUtc) } catch { return $false } }; Remove-Item -Path $o -Force -ErrorAction SilentlyContinue; if (M) { exit 0 }; if (-not [IO.File]::Exists($env:BIN)) { exit 2 }; $g=[guid]::NewGuid().ToString(); $n=$c + '.' + $g; try { [IO.File]::Copy($env:BIN, $n); if (-not (M)) { if ([IO.File]::Exists($c)) { [IO.File]::Move($c, $c + '.old.' + $g) }; [IO.File]::Move($n, $c) } } catch { }; Remove-Item -LiteralPath $n -Force -ErrorAction SilentlyContinue; Remove-Item -Path $o -Force -ErrorAction SilentlyContinue; if (M) { exit 0 }; exit 2"
if /i "%~1"=="worktree" if /i "%~2"=="hook" if exist "%DX_STATE%\dx.exe" set "BIN=%DX_STATE%\dx.exe"
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
set "DX_EXE="
for %%P in ("%DX_HERE%\..") do if /i "%%~nxP"=="execroot" if exist "%%~fP\DO_NOT_BUILD_HERE" set "DX_EXE=%%~fP\DO_NOT_BUILD_HERE"
if not defined DX_EXE goto dxchecked
if defined DX_REDIR goto dxnext
set "DX_SRC="
set /p DX_SRC=<"%DX_EXE%"
if not defined DX_SRC goto dxnext
for %%Q in ("%DX_SRC%") do set "DX_HERE=%%~fQ"
set "DX_REDIR=1"
goto dxup
:dxchecked
if /i "%DX_HERE%"=="%DX_SELF%" goto dxself
if not exist "%DX_HERE%\tools\dx.cmd" goto dxself
"%DX_HERE%\tools\dx.cmd" %*
:dxself
rem Worktree add and release run the last built dx without a stale check or a build. Keep in sync with tools/dx.
set "DX_FAST="
if /i "%~1"=="worktree" if /i "%~2"=="add" set "DX_FAST=1"
if /i "%~1"=="worktree" if /i "%~2"=="release" set "DX_FAST=1"
if defined DX_FAST if exist "%DX_STATE%\dx.exe" set "BIN=%DX_STATE%\dx.exe"
if defined DX_FAST if exist "%BIN%" goto run
if exist "%DX_STATE%\" if not exist "%DX_STATE%\CACHEDIR.TAG" (echo Signature: 8a477f597d28d172789f06886806bc55& echo # Rebuildable copy of dx kept by tools/dx.cmd; see https://bford.info/cachedir/) > "%DX_STATE%\CACHEDIR.TAG" 2>nul
call powershell -NoProfile -NonInteractive -Command "$s=$env:DX_STATE; $b=Join-Path $s 'built'; $i=Join-Path $s 'inputs'; if (-not ([IO.File]::Exists($env:BIN) -and [IO.File]::Exists($b) -and [IO.File]::Exists($i))) { exit 1 }; if ((Get-Item -LiteralPath $i).Length -eq 0) { exit 1 }; $t=[IO.File]::GetLastWriteTimeUtc($b); $p=@(Get-Content -LiteralPath $i | ForEach-Object { $_.TrimStart('/').TrimStart(':').Replace(':', '/') }) + @('.bazelrc', '.bazelversion', 'user.bazelrc', 'tools/bazelrc/correctness.bazelrc', 'tools/bazelrc/performance.bazelrc', 'tools/bazelrc/resources.bazelrc', 'MODULE.bazel', 'MODULE.bazel.lock', 'cargo-bazel-lock.json'); foreach ($f in $p) { if ([IO.File]::GetLastWriteTimeUtc((Join-Path $env:DX_ROOT $f)) -gt $t) { exit 1 } }; %DX_SYNC%"
if errorlevel 2 goto run
if errorlevel 1 goto dxbuild
set "BIN=%DX_STATE%\dx.exe"
goto run
:dxbuild
if not exist "%DX_STATE%" mkdir "%DX_STATE%"
if not exist "%DX_STATE%\CACHEDIR.TAG" (echo Signature: 8a477f597d28d172789f06886806bc55& echo # Rebuildable copy of dx kept by tools/dx.cmd; see https://bford.info/cachedir/) > "%DX_STATE%\CACHEDIR.TAG" 2>nul
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
set "DX_DC="
if not defined DX_RC call powershell -NoProfile -NonInteractive -Command "$w=[IO.Path]::GetFullPath($env:DX_ROOT); $seen=@{}; $global:g=$false; function U($t) { if ($t.Length -gt 1 -and ($t[0] -eq [char]34 -or $t[0] -eq [char]39) -and $t[-1] -eq $t[0]) { $t.Substring(1, $t.Length - 2) } else { $t } }; function S($f, $d) { if ($d -gt 16 -or $seen.ContainsKey($f)) { return $false }; $seen[$f]=1; try { $ls=[IO.File]::ReadAllLines($f) } catch { return $false }; foreach ($l in $ls) { $l=$l.Trim(); if ($l.StartsWith('#')) { continue }; $ws=@($l -split '\s+' | Where-Object { $_ } | ForEach-Object { U $_ }); if ($ws.Count -eq 0) { continue }; if ($ws[0] -ceq 'import' -or $ws[0] -ceq 'try-import') { $t=(U $l.Substring($ws[0].Length).Trim()).Replace(('{0}workspace{0}' -f [char]37), $w); if (-not [IO.Path]::IsPathRooted($t)) { $t=Join-Path $w $t }; if (S $t ($d + 1)) { return $true }; continue }; foreach ($t in $ws) { $t=$t.TrimStart([char]34, [char]39); if ($t.StartsWith('--disk_cache', 'Ordinal') -or $t.StartsWith('--nodisk_cache', 'Ordinal')) { return $true }; foreach ($n in @('--experimental_disk_cache_gc_max_size', '--experimental_disk_cache_gc_max_age')) { if ($t -ceq $n -or $t.StartsWith($n + '=', 'Ordinal')) { $global:g=$true } } } }; return $false }; $c=$env:XDG_CACHE_HOME; if (-not $c) { if ($env:HOME) { $c=Join-Path $env:HOME '.cache' } else { $c=$env:LOCALAPPDATA } }; if (-not $c) { exit 0 }; $h=$env:HOME; if (-not $h) { $h=$env:USERPROFILE }; $r=@(Join-Path $w '.bazelrc'); if ($h) { $r+=Join-Path $h '.bazelrc' }; if ($env:ProgramData) { $r+=Join-Path $env:ProgramData 'bazel.bazelrc' }; foreach ($f in $r) { if (S $f 0) { exit 0 } }; $o=[char]34 + '--disk_cache=' + (Join-Path (Join-Path $c 'dx') 'disk-cache') + [char]34; if (-not $global:g) { $o+=' --experimental_disk_cache_gc_max_size=20G --experimental_disk_cache_gc_max_age=14d' }; $o" > "%DX_STATE%\dc.%DX_TAG%"
if exist "%DX_STATE%\dc.%DX_TAG%" set /p DX_DC=<"%DX_STATE%\dc.%DX_TAG%"
del /q "%DX_STATE%\dc.%DX_TAG%" 2>nul
echo dx: building //:dx 1>&2
REM Exit 33 or 37 means the Bazel server crashed; retry once. Keep in sync with tools/dx.
:dxbuildrun
call bazel %DX_RC% build %DX_DC% --curses=no --show_progress_rate_limit=30 --progress_report_interval=30 --show_result=0 --ui_event_filters=-info //:dx 1>&2
set "DX_EC=%errorlevel%"
if not defined DX_BRETRY if "%DX_EC%"=="33" goto dxbcrash
if not defined DX_BRETRY if "%DX_EC%"=="37" goto dxbcrash
goto dxbdone
:dxbcrash
set "DX_BRETRY=1"
echo dx: the Bazel server crashed (exit %DX_EC%); trying //:dx again 1>&2
powershell -NoProfile -NonInteractive -Command "Start-Sleep 2"
goto dxbuildrun
:dxbdone
set "DX_FAIL=%DX_EC%"
set "DX_REPIN="
if not "%DX_FAIL%"=="0" call :dxwantsrepin %*
if "%DX_FAIL%"=="0" goto dxbuilt
if not defined DX_REPIN goto dxbuilt
echo dx: //:dx does not build; repinning the Cargo lock (CARGO_BAZEL_REPIN=1) and building it again 1>&2
setlocal
rem Keep in sync with the SetId::Cargo plan env in cli/update/src/backend.rs.
set "CARGO_BAZEL_REPIN=1"
call bazel %DX_RC% build %DX_DC% --curses=no --show_progress_rate_limit=30 --progress_report_interval=30 --show_result=0 --ui_event_filters=-info //:dx 1>&2
endlocal & set "DX_FAIL=%errorlevel%"
:dxbuilt
if not "%DX_FAIL%"=="0" (
  popd
  if not defined DX_REPIN echo dx: if a Cargo.toml dependency changed, run tools\dx update --apply cargo to repin 1>&2
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
call powershell -NoProfile -NonInteractive -Command "%DX_SYNC%"
if not errorlevel 1 set "DX_COPY=1"
:dxqueryrun
call bazel %DX_RC% query --noshow_progress --ui_event_filters=-info "kind('source file', deps(//:dx)) + buildfiles(deps(//:dx))" > "%DX_STATE%\query.%DX_TAG%"
set "DX_EC=%errorlevel%"
if not defined DX_QRETRY if "%DX_EC%"=="33" goto dxqcrash
if not defined DX_QRETRY if "%DX_EC%"=="37" goto dxqcrash
goto dxqdone
:dxqcrash
set "DX_QRETRY=1"
echo dx: the Bazel server crashed (exit %DX_EC%); trying //:dx again 1>&2
powershell -NoProfile -NonInteractive -Command "Start-Sleep 2"
goto dxqueryrun
:dxqdone
if not "%DX_EC%"=="0" (
  popd
  echo dx: bazel query for the inputs of //:dx failed; running the fresh dx anyway 1>&2
  del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
  goto dxcopy
)
popd
findstr /b /c:"//" "%DX_STATE%\query.%DX_TAG%" > "%DX_STATE%\inputs.%DX_TAG%"
del /q "%DX_STATE%\query.%DX_TAG%"
move /y "%DX_STATE%\inputs.%DX_TAG%" "%DX_STATE%\inputs" >nul
move /y "%DX_STATE%\built.%DX_TAG%" "%DX_STATE%\built" >nul
del /q "%DX_STATE%\*.%DX_TAG%" 2>nul
:dxcopy
if defined DX_COPY set "BIN=%DX_STATE%\dx.exe"
:run
"%BIN%" %*
exit /b %errorlevel%

:dxwantsrepin
if /i not "%~1"=="update" exit /b 0
set "DX_APPLY="
set "DX_SEL=0"
:dxwantsloop
shift
if "%~1"=="" goto dxwantsend
if /i "%~1"=="--apply" (set "DX_APPLY=1" & goto dxwantsloop)
if /i "%~1"=="--check" exit /b 0
if /i "%~1"=="--frozen" exit /b 0
if /i "%~1"=="--offline" exit /b 0
set "DX_WORD=%~1"
if "%DX_WORD:~0,1%"=="-" goto dxwantsloop
if /i not "%~1"=="cargo" exit /b 0
set /a DX_SEL+=1
goto dxwantsloop
:dxwantsend
if not defined DX_APPLY exit /b 0
if %DX_SEL% GTR 1 exit /b 0
set "DX_REPIN=1"
exit /b 0
