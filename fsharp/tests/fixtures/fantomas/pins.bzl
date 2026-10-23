"""Fantomas check plus fix wiring."""

FANTOMAS_LINE = "8.1.0 pinned"

FANTOMAS_ARTIFACT = "exact official tool package as declared DLLs over the managed .NET cohort"
FANTOMAS_RUNTIME = "shared managed .NET runtime cohort; no dotnet tool install"

FANTOMAS_CHECK = "fantomas check --json (exit 0 all unchanged, exit 99 with needs-formatting, exit 1 operational failure)"
FANTOMAS_FIX = "fantomas in-place format (re-read on exit 0, keep input otherwise)"
FANTOMAS_CONFIG_POLICY = "upstream built-in defaults without config, native .editorconfig interpretation with config"

FSHARP_FIXTURE_HELLO = "//fsharp/tests/fixtures/hello:hello_lib"

FANTOMAS_REJECTED = "dotnet tool install rejected; non-JSON console-parse rejected; auto-supplied preset rejected"
