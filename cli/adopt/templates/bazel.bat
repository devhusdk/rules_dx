@echo off
setlocal
if defined DX_BAZEL_CHILD goto bazel
set "DX_HOOK_EXIT=9009"
if not exist "%~dp0dx.cmd" goto hookfailed
set "DX_BAZEL_CHILD=1"
call "%~dp0dx.cmd" worktree hook bazel -- %* 1>&2
set "DX_HOOK_EXIT=%errorlevel%"
set "DX_BAZEL_CHILD="
if "%DX_HOOK_EXIT%"=="0" goto bazel
if "%DX_HOOK_EXIT%"=="77" exit /b 77
if "%DX_HOOK_EXIT%"=="130" exit /b 130
if "%DX_HOOK_EXIT%"=="143" exit /b 143
:hookfailed
echo tools\bazel.bat: tools\dx.cmd worktree hook bazel failed (exit %DX_HOOK_EXIT%); running Bazel without it 1>&2
:bazel
"%BAZEL_REAL%" %*
exit /b %errorlevel%
