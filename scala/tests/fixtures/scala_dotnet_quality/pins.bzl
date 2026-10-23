"""Scala +.NET quality defaults pins."""

SCALAFMT_VERSION = "3.11.4"
CSHARPIER_VERSION = "1.3.0"
FANTOMAS_LINE = "7.x stable"
FSHARPLINT_VERSION = "0.27.0"

SCALAFMT_ARTIFACT = "exact official native binary"
CSHARPIER_ARTIFACT = "exact official tool package as declared DLLs (targets .NET 8.0)"
FANTOMAS_ARTIFACT = "exact official tool package as declared DLLs over the managed .NET cohort"
FSHARPLINT_ARTIFACT = "exact official tool package over the managed .NET cohort (targets .NET 8.0)"
ROSLYN_COUPLING = "SDK-built-in CA analyzers following the qualified .NET SDK, no separate acquisition"

REJECTED_SCALAFMT_HEAD = "newer than 3.11.4 observed, not pinned; living at head rejected"
REJECTED_FANTOMAS_ALPHA = "8.0.0 alphas target the next FSharp.Core/.NET and are not stable; rejected"

NATIVE_CONFIG_POLICY = "native-configuration sole policy: no hidden presets"

ROSLYN_POLICY = "SDK default analysis mode is the upstream built-in default mode, not a rules_dx preset"
STYLECOP_POLICY = "StyleCop stays opt-in, never the default; its style rules can contradict the built-in IDE rules"
FSHARPLINT_POLICY = "default ruleset with formatting rules off is the upstream built-in default ruleset minus formatting; Fantomas owns formatting"
BEYOND_DEFAULT_REJECTED = "beyond-default switches rejected: auto preset, --enable=all-style opt-in maxima"
AUTO_PRESET_REJECTED = "auto preset rejected: provisional inputs only, never an automatically supplied config"

SCALA_FIXTURE_HELLO = "//scala/tests/fixtures/hello:hello_test"
CSHARP_FIXTURE_HELLO = "//csharp/tests/fixtures/hello:hello_test"
FSHARP_FIXTURE_HELLO = "//fsharp/tests/fixtures/hello:hello_lib"

SCALA_DOTNET_REJECTED = "hidden presets rejected: no auto-supplied Scalafix preset, no StyleCop default, no FSharpLint formatting preset; unpinned versions rejected: no floating version or head"
