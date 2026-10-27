"""Windows launcher text for starlark_test."""

def windows_sh_path_error(sh_path):
    """Returns an error message when sh_path cannot be quoted in a .bat, else the empty string."""
    for bad in ['"', "%", "!"]:
        if bad in sh_path:
            return "starlark_test: the sh path " + repr(sh_path) + " contains '" + bad + "', which a .bat launcher cannot quote; use a shell path without it"
    return ""

def windows_launcher_body(runner_key, sh_path):
    """Returns the CRLF .bat launcher text that runs the runner under sh."""
    lines = [
        "@echo off",
        "setlocal ENABLEEXTENSIONS ENABLEDELAYEDEXPANSION",
        "set \"RUNNER=" + runner_key.replace("\\", "/") + "\"",
        "if exist \"%TEST_SRCDIR%\\MANIFEST\" set \"M=%TEST_SRCDIR%\\MANIFEST\"",
        "if not defined M if exist \"%~f0.runfiles\\MANIFEST\" set \"M=%~f0.runfiles\\MANIFEST\"",
        "if not defined M set \"M=%~f0.runfiles_manifest\"",
        "for /F \"tokens=2* usebackq\" %%i in (`%SYSTEMROOT%\\system32\\findstr.exe /b /l /c:\"!RUNNER! \" \"%M%\"`) do set \"SCRIPT=%%i\"",
        "if not defined SCRIPT echo>&2 ERROR: !RUNNER! not found in the runfiles manifest & exit /b 1",
    ]
    if sh_path:
        lines.append("set \"DX_SH=" + sh_path.replace("/", "\\") + "\"")
        lines.append("if not exist \"!DX_SH!\" set \"DX_SH=\"")
    lines += [
        "if not defined DX_SH if defined BAZEL_SH if exist \"!BAZEL_SH!\" set \"DX_SH=!BAZEL_SH!\"",
        "if not defined DX_SH goto :dx_path_sh",
        "for %%d in (\"!DX_SH!\") do set \"DX_DIR=%%~dpd\"",
        "set \"PATH=!DX_DIR!;!DX_DIR!..\\usr\\bin;%PATH%\"",
        "\"!DX_SH!\" \"%SCRIPT:\\=/%\" %*",
        "exit /b %ERRORLEVEL%",
        ":dx_path_sh",
        "%SYSTEMROOT%\\system32\\where.exe sh >nul 2>&1",
        "if errorlevel 1 echo>&2 ERROR: no sh found; set BAZEL_SH to bash.exe & exit /b 1",
        "sh \"%SCRIPT:\\=/%\" %*",
        "exit /b %ERRORLEVEL%",
    ]
    return "\r\n".join(lines) + "\r\n"
