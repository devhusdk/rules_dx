"""Public Elixir API."""

load(
    "//elixir/private:defs.bzl",
    _escript_archive = "escript_archive",
    _escript_headers = "escript_headers",
    _elixir_app = "elixir_app",
    _elixir_test = "elixir_test",
    _elixir_bench = "elixir_bench",
)

escript_archive = _escript_archive
escript_headers = _escript_headers
elixir_app = _elixir_app
elixir_test = _elixir_test
elixir_bench = _elixir_bench
