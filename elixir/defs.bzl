"""Public Elixir API."""

load(
    "//elixir/private:defs.bzl",
    _COVER_LOAD = "COVER_LOAD",
    _ELIXIR_CELLS = "ELIXIR_CELLS",
    _ELIXIR_COMPATIBLE_WITH = "ELIXIR_COMPATIBLE_WITH",
    _ELIXIR_DIALYZER_COMPATIBLE_WITH = "ELIXIR_DIALYZER_COMPATIBLE_WITH",
    _ELIXIR_SOURCE_GLOBS = "ELIXIR_SOURCE_GLOBS",
    _escript_archive = "escript_archive",
    _escript_headers = "escript_headers",
    _cover_sources = "cover_sources",
    _app_sources = "app_sources",
    _nif_priv = "nif_priv",
    _elixir_app = "elixir_app",
    _helper_first = "helper_first",
    _elixir_test = "elixir_test",
    _elixir_bench = "elixir_bench",
)

COVER_LOAD = _COVER_LOAD
ELIXIR_CELLS = _ELIXIR_CELLS
ELIXIR_COMPATIBLE_WITH = _ELIXIR_COMPATIBLE_WITH
ELIXIR_DIALYZER_COMPATIBLE_WITH = _ELIXIR_DIALYZER_COMPATIBLE_WITH
ELIXIR_SOURCE_GLOBS = _ELIXIR_SOURCE_GLOBS
escript_archive = _escript_archive
escript_headers = _escript_headers
cover_sources = _cover_sources
app_sources = _app_sources
nif_priv = _nif_priv
elixir_app = _elixir_app
helper_first = _helper_first
elixir_test = _elixir_test
elixir_bench = _elixir_bench
