# Contributing

`rules_dx` is a Bazel developer platform. `dx` runs through Bazel.

```sh
tools/dx worktree add <branch>
tools/dx fix --apply --affected
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

Use the bug template. Paste `dx version` and `dx status`.

Each issue gets one priority label:

- `P0`: agent dev loop (worktrees, slots, fix, check, CI, launcher, JSON).
- `P1`: bugs in existing features.
- `P2`: improve and finish existing features.
- `P3`: new languages, platforms and features, last.

Other labels: `bug`, `enhancement`, `needs-host` (needs macOS, Windows, GPU, or signing secrets), `tracking`.
`dogfood` is an extra label, never the only one.

Filters: [user issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+-label%3Adogfood), [dogfood issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+label%3Adogfood).

## Dependency floors

Every consumer-facing (non-dev) `bazel_dep` in the root `MODULE.bazel` has two
versions, both recorded in `modules/versions.bzl`:

- `FLOOR_<DEP>`, in the `bazel_dep`: the oldest version consumers may resolve.
- `TESTED_<DEP>`, in a root-only `single_version_override` directly below it:
  the version the root builds and tests with. Consumers never see it.

A `# floor: <reason>` comment directly above each `bazel_dep` says why the floor
is what it is. Tools parse that marker, so keep it as written. Use
`dx bump bazel:<dep> <version>` to move the tested version, and
`dx bump bazel:<dep> <version> --floor --reason "<why>"` to move the floor.
`//modules:consumer_deps_test` checks that the two files agree.

### Where floors are checked

rules_dx is a non-root module in every consumer, so its root-only
`single_version_override`s do not apply there and a consumer resolves the floors.
Three checks cover that:

- `//modules:floor_graph_test` runs `dx init` in a scratch consumer and asserts
  through `bazel mod graph` that every table dep resolves at its floor. When one
  resolves higher, the failure names the module that requested that version.
  It runs on every PR in the `network` job of `ci`. Patched deps are skipped,
  because the `dx init` include pins them.
- `//modules:consumer_deps_test` also keeps the hermetic consumer
  `module_fragment.bazel` files and the inline module of `consumer_names_test.rs`
  from pinning a consumer dep above its floor, which would hide a lowered floor.
- `floor.yml` repeats the graph check nightly, once at the floors and once with
  every dep at its tested version (`DX_CONSUMER_OVERRIDES=tested`).

### Trying a lower floor

Set `DX_CONSUMER_OVERRIDES=<module>=<version>,...` to resolve candidate versions
instead. Each one becomes the expected version, is written as a root
`single_version_override`, and rewrites the consumer's own `bazel_dep` of that
module. Locally:

    bazel test //modules:floor_graph_test --test_tag_filters= \
      --test_env=DX_CONSUMER_OVERRIDES=rules_shell=0.6.1

Or dispatch `floor.yml` with the `overrides` input. Patched modules cannot be
overridden. The procedure for a floor search is: list the module's versions in
the BCR, step down one release at a time, check that the `.bzl` files rules_dx
loads from it still find the symbols they need, and keep the lowest version that
passes the graph check and the consumer suites. Record why in the `# floor:`
comment with `dx bump ... --floor --reason`. Skip dev-only and patched deps.

To run the hermetic consumer suites at candidate versions, add
`--test_env=DX_CONSUMER_OVERRIDES=<module>=<version>` to a `hermetic-consumer`
test (the harness rewrites each consumer `MODULE.bazel` the same way), or dispatch
`floor.yml` with `overrides` and `consumer_target`.

For now every floor equals its tested version. Do not lower a floor yet: the
floor searches themselves are still follow-ups to #3747.

`.dx/launcher` is a copy (about 50 MB) of the last built dx, kept for worktree hooks and failed rebuilds. `tools/dx worktree add`, `release` and `hook` run the last built dx without rebuilding, so they never wait on Bazel. Any other `tools/dx` command (or `bazel build //:dx`) picks up dx source changes. You can delete it any time and `tools/dx` recreates it. `tools/dx worktree prune --apply --bases` drops it in free slots. Both launchers run dx from this copy, so a running dx never locks bazel-bin; `dx.exe.old.*` files may remain while an old dx is still running.
