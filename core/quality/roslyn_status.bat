@echo off
setlocal
set "STATUS=%~1"
set "WRAPPER=%~2"
set "WRAPPER=%WRAPPER:/=\%"
call "%WRAPPER%" %3 %4 %5
>"%STATUS%" echo %ERRORLEVEL%
exit /b 0
