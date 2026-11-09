# Zig

Zig test fixtures. They live in the `rules_dx_zig` module, next to `rules_dx`.
The rules, the tools, the lint, format and typecheck aspects and the Gazelle
language stay in `rules_dx`. The module registers the Zig toolchain and fetches
the xxHash source the fixtures use, so a workspace that does not use Zig does
not fetch them.
