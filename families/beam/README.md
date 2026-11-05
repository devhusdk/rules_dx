# BEAM

Elixir and Erlang tests and fixtures. They live in the `rules_dx_beam` module, a
`bazel_dep` next to `rules_dx`. The rules, the Erlang and Elixir toolchains and
the quality tools stay in `rules_dx`, because its own tool targets use them.

- [Elixir](elixir/README.md)
- [Erlang](erlang/README.md)
