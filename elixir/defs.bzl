"""Public Elixir API."""

load(
    "//elixir/private:defs.bzl",
    _elixir_escript_archive = "elixir_escript_archive",
    _elixir_escript_headers = "elixir_escript_headers",
    _elixir_app = "elixir_app",
    _elixir_test = "elixir_test",
    _elixir_bench = "elixir_bench",
)
load("//tools/docs/elixir:defs.bzl", _elixir_doc_shard = "elixir_doc_shard")

elixir_escript_archive = _elixir_escript_archive
elixir_escript_headers = _elixir_escript_headers
elixir_app = _elixir_app
elixir_test = _elixir_test
elixir_bench = _elixir_bench
elixir_doc_shard = _elixir_doc_shard
