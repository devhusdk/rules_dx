# Contributing

`rules_dx` is a Bazel developer platform. `dx` runs through Bazel.

```sh
tools/dx worktree add <branch>
tools/dx check --apply --affected
tools/dx test --affected
tools/dx worktree release
```

Full run: `tools/dx test //...`

Read [AGENTS.md](AGENTS.md) before changing code.

- Treat warnings as errors.
- Run the formatter, linter, and focused tests for your change.
- Update tests and user docs when behavior changes.
- Do not edit generated files. Use the generator.
- Library crates return typed errors. Only binaries map exit codes.

## Issues and labels

Use the bug template. Paste `dx status`.

Each issue gets one priority label:

- `P0`: agent dev loop (worktrees, slots, fix, check, CI, launcher, JSON).
- `P1`: bugs in existing features.
- `P2`: improve and finish existing features.
- `P3`: new languages, platforms and features, last.

Other labels: `bug`, `enhancement`, `needs-host` (needs macOS, Windows, GPU, or signing secrets), `tracking`.
`dogfood` is an extra label, never the only one.

Filters: [user issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+-label%3Adogfood), [dogfood issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+label%3Adogfood).

## Dependency floors

Every consumer-facing (non-dev) `bazel_dep` in the root `MODULE.bazel` has its
versions in `modules/versions.bzl`:

- `<DEP>_VERSION` when the floor and the tested version are the same.
- `FLOOR_<DEP>` and `TESTED_<DEP>` when they differ. The floor goes in the
  `bazel_dep`, the tested version in the root-only `single_version_override`
  directly below it. Consumers never see the tested version.

A `# floor: <reason>` comment directly above each `bazel_dep` says why the floor
is what it is. Tools parse that marker, so keep it as written. Use
`dx update bazel:<dep> <version>` to move the tested version, and
`dx update bazel:<dep> <version> --floor --reason "<why>"` to move the floor.
`//modules:consumer_deps_test` checks that the two files agree.

### Where floors are checked

rules_dx is a non-root module in every consumer, so its root-only
`single_version_override`s do not apply there and a consumer resolves the floors.
`//modules:consumer_deps_test` keeps the hermetic consumer `module_fragment.bazel`
files from pinning a consumer dep above its floor, which would hide a lowered floor.

### Trying a lower floor

Set `DX_CONSUMER_OVERRIDES=<module>=<version>,...` to resolve candidate versions
instead. The harness writes each one as a root `single_version_override` and rewrites
the consumer's own `bazel_dep` of that module. The procedure for a floor search is: list the module's versions in
the BCR, step down one release at a time, check that the `.bzl` files rules_dx
loads from it still find the symbols they need, and keep the lowest version that
passes the consumer suites. Record why in the `# floor:`
comment with `dx update ... --floor --reason`. Skip dev-only and patched deps.

To run the hermetic consumer suites at candidate versions, add
`--test_env=DX_CONSUMER_OVERRIDES=<module>=<version>` to a `hermetic-consumer`
test (the harness rewrites each consumer `MODULE.bazel` the same way).

A tested-only bump leaves the floor at the previous tested version, and its
comment says so. Do not lower a floor without a floor search; the searches
themselves are still follow-ups to #3747.

`.dx/launcher` is a copy (about 50 MB) of the last built dx, kept for worktree hooks and failed rebuilds. `tools/dx worktree add`, `release` and `hook` run the last built dx without rebuilding, so they never wait on Bazel. Any other `tools/dx` command (or `bazel build //:dx`) picks up dx source changes. You can delete it any time and `tools/dx` recreates it. `tools/dx worktree prune --apply --bases` drops it in free slots. Both launchers run dx from this copy, so a running dx never locks bazel-bin; `dx.exe.old.*` files may remain while an old dx is still running.
