@echo off
setlocal
set "VERSION=0.0.0"
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
set "DX_DC="
if not defined DX_RC call powershell -NoProfile -NonInteractive -Command "$w=[IO.Path]::GetFullPath($env:DX_ROOT); $seen=@{}; function U($t) { if ($t.Length -gt 1 -and ($t[0] -eq [char]34 -or $t[0] -eq [char]39) -and $t[-1] -eq $t[0]) { $t.Substring(1, $t.Length - 2) } else { $t } }; function S($f, $d) { if ($d -gt 16 -or $seen.ContainsKey($f)) { return $false }; $seen[$f]=1; try { $ls=[IO.File]::ReadAllLines($f) } catch { return $false }; foreach ($l in $ls) { $l=$l.Trim(); if ($l.StartsWith('#')) { continue }; $ws=@($l -split '\s+' | Where-Object { $_ } | ForEach-Object { U $_ }); if ($ws.Count -eq 0) { continue }; if ($ws[0] -ceq 'import' -or $ws[0] -ceq 'try-import') { $t=(U $l.Substring($ws[0].Length).Trim()).Replace(('{0}workspace{0}' -f [char]37), $w); if (-not [IO.Path]::IsPathRooted($t)) { $t=Join-Path $w $t }; if (S $t ($d + 1)) { return $true }; continue }; foreach ($t in $ws) { $t=$t.TrimStart([char]34, [char]39); if ($t.StartsWith('--disk_cache', 'Ordinal') -or $t.StartsWith('--nodisk_cache', 'Ordinal')) { return $true } } }; return $false }; $c=$env:XDG_CACHE_HOME; if (-not $c) { if ($env:HOME) { $c=Join-Path $env:HOME '.cache' } else { $c=$env:LOCALAPPDATA } }; if (-not $c) { exit 0 }; $h=$env:HOME; if (-not $h) { $h=$env:USERPROFILE }; $r=@(Join-Path $w '.bazelrc'); if ($h) { $r+=Join-Path $h '.bazelrc' }; if ($env:ProgramData) { $r+=Join-Path $env:ProgramData 'bazel.bazelrc' }; foreach ($f in $r) { if (S $f 0) { exit 0 } }; [char]34 + '--disk_cache=' + (Join-Path (Join-Path $c 'dx') 'disk-cache') + [char]34" > "%DX_STATE%\dc.%DX_TAG%"
if exist "%DX_STATE%\dc.%DX_TAG%" set /p DX_DC=<"%DX_STATE%\dc.%DX_TAG%"
del /q "%DX_STATE%\dc.%DX_TAG%" 2>nul
echo dx: building //:dx 1>&2
call bazel %DX_RC% build %DX_DC% --curses=no --show_progress_rate_limit=30 --progress_report_interval=30 --show_result=0 --ui_event_filters=-info //:dx 1>&2
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
mkdir "%DIR%" 2>nul
set "DX_TAG="
for /f %%G in ('powershell -NoProfile -NonInteractive -Command "[guid]::NewGuid().ToString()"') do set "DX_TAG=%%G"
if not defined DX_TAG (
  echo dx: download failed: %DX_URL% 1>&2
  goto bazel
)
set "DX_PART=%DIR%\.dx-%ARCH%.%DX_TAG%.part"
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
del /q "%DX_PART%" "%DX_PART%.sha" 2>nul
exit /b 0
:bazel
echo dx: run it through Bazel instead: bazel run @rules_dx//:dx -- %* 1>&2
exit /b 1
