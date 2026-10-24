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
call powershell -NoProfile -NonInteractive -Command "$s=$env:DX_STATE; $b=Join-Path $s 'built'; $i=Join-Path $s 'inputs'; if (-not ([IO.File]::Exists($env:BIN) -and [IO.File]::Exists($b) -and [IO.File]::Exists($i))) { exit 1 }; $t=[IO.File]::GetLastWriteTimeUtc($b); $p=@(Get-Content -LiteralPath $i | ForEach-Object { $_.TrimStart('/').TrimStart(':').Replace(':', '/') }) + @('.bazelrc', '.bazelversion', 'user.bazelrc', 'tools/bazelrc/preset.bazelrc', 'MODULE.bazel', 'MODULE.bazel.lock', 'cargo-bazel-lock.json'); foreach ($f in $p) { if ([IO.File]::GetLastWriteTimeUtc((Join-Path $env:DX_ROOT $f)) -gt $t) { exit 1 } }; exit 0"
if not errorlevel 1 goto run
if not exist "%DX_STATE%" mkdir "%DX_STATE%"
del /q "%DX_STATE%\built.new" 2>nul
type nul > "%DX_STATE%\built.new"
pushd "%DX_ROOT%"
set "DX_RC=--nohome_rc --nosystem_rc"
if not "%CI%"=="true" if "%BAZEL_TEST%"=="" set "DX_RC="
call bazel %DX_RC% build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx 1>&2
if errorlevel 1 (
  popd
  echo dx: bazel build //:dx failed 1>&2
  exit /b 1
)
call bazel %DX_RC% query --noshow_progress --ui_event_filters=-info "kind('source file', deps(//cli/cli:dx)) + buildfiles(deps(//cli/cli:dx))" > "%DX_STATE%\query"
if errorlevel 1 (
  popd
  echo dx: bazel query for the inputs of //cli/cli:dx failed 1>&2
  exit /b 1
)
popd
findstr /b /c:"//" "%DX_STATE%\query" > "%DX_STATE%\inputs.new"
del /q "%DX_STATE%\query"
move /y "%DX_STATE%\inputs.new" "%DX_STATE%\inputs" >nul
move /y "%DX_STATE%\built.new" "%DX_STATE%\built" >nul
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
