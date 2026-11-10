"""Public Elixir API."""

load(
    "//elixir/private:defs.bzl",
    _ELIXIR_CELLS = "ELIXIR_CELLS",
    _ELIXIR_COMPATIBLE_WITH = "ELIXIR_COMPATIBLE_WITH",
    _ELIXIR_DIALYZER_COMPATIBLE_WITH = "ELIXIR_DIALYZER_COMPATIBLE_WITH",
    _ELIXIR_SOURCE_GLOBS = "ELIXIR_SOURCE_GLOBS",
    _escript_archive = "escript_archive",
    _escript_headers = "escript_headers",
    _elixir_app = "elixir_app",
    _elixir_test = "elixir_test",
    _elixir_bench = "elixir_bench",
)

ELIXIR_CELLS = _ELIXIR_CELLS
ELIXIR_COMPATIBLE_WITH = _ELIXIR_COMPATIBLE_WITH
ELIXIR_DIALYZER_COMPATIBLE_WITH = _ELIXIR_DIALYZER_COMPATIBLE_WITH
ELIXIR_SOURCE_GLOBS = _ELIXIR_SOURCE_GLOBS
escript_archive = _escript_archive
escript_headers = _escript_headers
elixir_app = _elixir_app
elixir_test = _elixir_test
elixir_bench = _elixir_bench
