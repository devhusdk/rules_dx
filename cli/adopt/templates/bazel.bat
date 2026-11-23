@echo off
setlocal
if defined DX_BAZEL_CHILD goto bazel
if not exist "%~dp0dx.cmd" goto bazel
set "DX_BAZEL_CHILD=1"
call "%~dp0dx.cmd" worktree hook bazel -- %* 1>&2
if errorlevel 1 exit /b %errorlevel%
set "DX_BAZEL_CHILD="
:bazel
"%BAZEL_REAL%" %*
exit /b %errorlevel%
