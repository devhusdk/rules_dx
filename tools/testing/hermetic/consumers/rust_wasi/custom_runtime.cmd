@echo off
setlocal
set "subcommand=%~1"
for /f "tokens=1,* delims= " %%a in ("%*") do set "rest=%%b"
"%DX_WASI_MANAGED_RUNTIME%" %subcommand% --env DX_WASI_RUNTIME_NOTE=custom %rest%
exit /b %errorlevel%
