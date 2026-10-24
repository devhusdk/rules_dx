@echo off
setlocal
set "VERSION=0.0.0"
set "ARCH=x86_64"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ARCH=arm64"
if not exist "%~dp0..\cli\cli\BUILD.bazel" goto pinned
findstr /b /c:"    name = \"rules_dx\"," "%~dp0..\MODULE.bazel" >nul 2>&1
if errorlevel 1 goto pinned
pushd "%~dp0.."
call bazel build --noshow_progress --show_result=0 --ui_event_filters=-info //:dx 1>&2
if errorlevel 1 (
  popd
  echo dx: bazel build //:dx failed 1>&2
  exit /b 1
)
set "BAZEL_BIN="
for /f "delims=" %%B in ('bazel info bazel-bin 2^>nul') do set "BAZEL_BIN=%%B"
popd
"%BAZEL_BIN%\cli\cli\dx.exe" %*
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
