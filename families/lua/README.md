# Lua

Lua test fixtures. They live in the `rules_dx_lua` module, next to `rules_dx`.
The rules, the toolchain, the tools, the lint, format and typecheck aspects and
the Gazelle language stay in `rules_dx`. The module registers the Lua
toolchain and fetches the inspect.lua source the fixtures use, so a workspace
that does not use Lua does not fetch them.
