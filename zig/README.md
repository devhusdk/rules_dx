# Zig

This directory helps you build, run and test Zig code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//zig:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Zig page](../docs/languages/zig.md) shows
setup, every rule and the tools dx runs.

Run the Zig tests from a rules_dx checkout:

```sh
tools/dx test //zig/...
```

## Dependencies

### Packages

The `.dependencies` of your `build.zig.zon` become one hub of `zig_library`
targets. A hub is a Bazel repository that holds one target per locked
package. The hub reads `zig.lock.json`, and Bazel fetches each archive
through its repository cache. Zig never runs at fetch time.

To lock the packages, run `dx update`. It writes `zig.lock.json` beside
`build.zig.zon`. A second run with unchanged inputs downloads nothing. Then
run `dx generate --apply`: it adds the `zig.lock` stanza for the `@zig` hub
to `MODULE.bazel`.

```sh
dx update --apply
dx generate --apply
```

Import a package as `@zig//<name>` and `@import("<name>")`. Its root module is
the first of these files that exists: `src/<name>.zig`, `src/root.zig`,
`<name>.zig`, `src/main.zig`, `root.zig`, `main.zig`. Its own `build.zig.zon`
dependencies become its deps, and they must be in your root `build.zig.zon`
with the same `.hash`.

Only `https` archive URLs (`.tar.gz`, `.tgz`, `.zip`) work. `git+` URLs and
path dependencies do not.

### Writing targets

- `main` is the module root. `srcs` lists the other `.zig` files.
- Other code imports a `zig_library` by its target name. Set `import_name` to
  change it.
- A `cc_library` in `deps` is imported as `@import("c")`.
- `extra_srcs` holds files read with `@embedFile`.
- Other attributes pass through to `rules_zig`.

`zig_binary` and `zig_test` build in ReleaseSafe, also under `-c opt`.
Coverage runs build tests in Debug, which keeps the line data.
`--@rules_zig//zig/settings:mode` picks another mode. ReleaseFast, from that
flag or from `zigopts`, fails unless the target sets
`allow_release_fast = True`.

## Generate

`dx generate` writes one `zig_library` per directory:

- The root is `<directory>.zig`, `root.zig`, `lib.zig` or the only `.zig`
  file. The other `.zig` files go to `srcs`.
- `*_test.zig` and `build.zig` are skipped.
- The root package is named after `.name` in `build.zig.zon`, else the repo
  name, else `lib`.
- `@import("name")` adds the library whose import name is `name`. `@import` of
  `std`, `builtin`, `root`, `bazel_builtin` and `c` adds nothing.
- `.zon` imports and `@embedFile` files in the same directory go to
  `extra_srcs`.

Generation fails when a file import or `@embedFile` points outside the
directory, is missing, or names a `*_test.zig` or `build.zig` file. It also
fails for a directory with `pub fn main`: write its `zig_binary` by hand.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `zig_fmt` |
| Lint | `zig_ast_check`, `typos` |
| Typecheck | none |

The tools need no setup. Bazel fetches the Zig SDK only for Zig targets.

```sh
dx format --apply
dx lint
```

`zig_fmt` runs `zig fmt`. `zig_ast_check` runs `zig ast-check` once per file
and reports syntax and AST errors. In the strict tier it also fails on
`catch unreachable`, `catch {}`, `std.debug.print` outside tests,
`@panic("TODO...")`, and `@ptrCast`, `@alignCast` or `@intCast` without a
`// SAFETY:` comment. `docs/cli/commands/quality.md` lists the rule IDs.

To typecheck, turn on `zig` in `dx.toml`:

```toml
[families.zig]
typecheck = ["+zig"]
```

`dx typecheck` then replays the compile action of each `zig_binary` and
`zig_test` with `-fno-emit-bin`. Zig analyzes lazily, so it checks only code
reachable from the root module, and never checks a `zig_library` on its own.

## Coverage

`bazel coverage` and `dx coverage` run each Zig test under kcov v43, which
rules_dx builds from source, and turn its Cobertura report into LCOV. Under
coverage, the test builds with LLVM and keeps its debug info so kcov can read
it. Coverage counts the sources of libraries in `deps`. The test's own sources
count only with `--instrument_test_targets`. Tag a test `no-coverage` to skip
it.

kcov runs on Linux only. On macOS and Windows a Zig test is tagged
`dx-no-coverage-collector` and writes no coverage. Zig cannot emit
`-fprofile-instr-generate`, so LLVM source-based coverage is not available.

## Bench and fuzz

`zig_bench` builds a benchmark for `dx bench`:

```python
load("@rules_dx//zig:defs.bzl", "zig_bench")

zig_bench(
    name = "parse_bench",
    main = "parse_bench.zig",
)
```

The program times its own loop with `std.Io.Timestamp` and prints one line per
benchmark: `bench <name> <ns_per_op> <iterations>`. The test turns those lines
into `dx-bench.json`.

`<name>` is the `dx-bench` test that `dx bench` runs, and `<name>_bin` is the
binary, built with `mode = "release_fast"`. `size`, `timeout`, `args`, `data`
and `env` go to the test. The other attributes go to the binary.

There is no `zig_fuzz_test`. Zig's fuzzer runs only under `zig build --fuzz`,
and a Bazel-built test binary cannot run it with a time limit and crash
output.

## API docs

`dx docs` documents every `zig_library` in scope. It reads each source with
`std.zig.Ast` from the pinned Zig and turns the `///` and `//!` comments of
the public declarations into the API reference. Files that the main file
re-exports with `pub const x = @import("x.zig")` become modules. You install
nothing. To declare a docs target yourself, use `zig_doc_shard` from
`@rules_dx//zig:defs.bzl`.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
