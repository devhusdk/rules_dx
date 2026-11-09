# Zig

Build, run, test and format Zig with the pinned `rules_zig` 0.17.0 and Zig
0.16.0 toolchain. Nothing needs to be installed.

```python
load("@rules_dx//zig/rules:defs.bzl", "zig_binary", "zig_library", "zig_test")

zig_library(
    name = "greet",
    main = "greet.zig",
)

zig_binary(
    name = "hello",
    main = "main.zig",
    deps = [":greet"],
)

zig_test(
    name = "greet_test",
    main = "greet.zig",
)
```

- `main` is the module root. `srcs` lists the other `.zig` files.
- `zig_library` imports by its target name. Set `import_name` to change it.
- A `cc_library` in `deps` is imported as `@import("c")`.
- `extra_srcs` carries files read with `@embedFile`.
- Other attributes pass through to `rules_zig`.

Start a project with `dx init --apply demo` and add sources.

## API docs

`dx docs` documents every `zig_library` in scope. A small Zig program built with
the pinned Zig reads each source with `std.zig.Ast`, and `zig_doc_shard`
(`@rules_dx//tools/docs/zig:defs.bzl`) turns the `///` and `//!` comments of the
public declarations into the API reference. Own files the main file re-exports
with `pub const x = @import("x.zig")` become modules. Nothing needs to be
installed.

## Packages

The `.dependencies` of a `build.zig.zon` become one hub of `zig_library` targets. Bazel fetches each archive through its repository cache; Zig never runs at fetch time. Only `http(s)` archive URLs (`.tar.gz`, `.tgz`, `.tar.xz`, `.tar.zst`, `.zip`) are supported; `git+` URLs and path dependencies are not.

```python
zig_deps = use_extension("@rules_dx//zig/deps:extensions.bzl", "zig_deps")
zig_deps.lock(
    name = "zig_packages",
    digests = "//:zig.digests.json",
    lock = "//:build.zig.zon",
)
use_repo(zig_deps, "zig_packages")
```

A package is `@zig_packages//<name>`, imported by that name. Its root module is the first existing of `src/<name>.zig`, `src/root.zig`, `<name>.zig`, `src/main.zig`, `root.zig`, `main.zig`. Its own `build.zig.zon` dependencies that the hub holds become deps. `@zig_packages//:all_packages` groups every package.

The `.hash` of a dependency is not the archive digest, so `zig.digests.json` pins the archives:

```json
{
  "packages": {
    "xxhash": {
      "hash": "<the .hash in build.zig.zon>",
      "sha256": "<sha256 of the archive at .url>"
    }
  }
}
```

A package without a matching entry fails the fetch and prints the entry to commit.

## Benchmarks

`zig_bench` builds a private `<name>_bin` with `mode = "release_fast"` and declares `<name>` as the manual `dx-bench` test `dx bench` runs. Test attributes (`size`, `timeout`, `args`, `data`, `env`) go to `<name>`, the other attributes to the binary.

```python
load("@rules_dx//zig/rules:defs.bzl", "zig_bench")

zig_bench(
    name = "parse_bench",
    main = "parse_bench.zig",
)
```

The program times its own loop with `std.Io.Timestamp` and prints one line per benchmark, `bench <name> <ns_per_op> <iterations>`. The test turns those lines into `dx-bench.json`.

There is no `zig_fuzz_test`. Zig's fuzzer runs only under `zig build --fuzz`, which drives the test binary over a private protocol. A Bazel-built test binary cannot run it with a time limit and crash output.

## Formatting

`dx format` runs `zig fmt` by default. No flag is needed. The Zig SDK is fetched only for Zig targets:

```sh
dx format --apply
```

## Typechecking

`dx typecheck` replays the compile action of a `zig_binary` or `zig_test` with
`-fno-emit-bin` when a `zig` family selects `zig`. Zig analyzes lazily, so only
code reachable from the root module is checked, and `zig_library` is not
checked on its own.

## Linting

`dx lint` runs `zig ast-check` by default. No flag is needed. It runs once per file and reports syntax and AST-level errors:

```sh
dx lint
```

## Generating BUILD files

`dx generate` writes one `zig_library` per directory. The root is
`<directory>.zig`, `root.zig`, `lib.zig` or the only `.zig` file. Other `.zig`
files go to `srcs`. `*_test.zig` and `build.zig` are skipped. The root package is named
after `.name` in `build.zig.zon`, else the repo name, else `lib`. `@import("name")` adds the library whose import
name is `name`. `@import` of `std`, `builtin`, `root`, `bazel_builtin` and
`c` adds nothing. `.zon` imports and `@embedFile` files in the same directory
go to `extra_srcs`. A file import or `@embedFile` that points outside the
directory, is missing, or names a `*_test.zig` or `build.zig` file fails
generation. A directory with `pub fn main` fails generation: write its
`zig_binary` by hand.

## Platforms

| Host | Zig SDK | Qualified |
| --- | --- | --- |
| Linux x86_64 | yes | yes |
| Linux arm64 | yes | no |
| macOS x86_64, arm64 | yes | no |
| Windows x86_64 | yes | no |
| Windows arm64 | yes | no |

## Coverage

`bazel coverage` and `dx coverage` run each Zig test under a hermetic kcov v43 that rules_dx builds from source, and convert its Cobertura report into LCOV. Under coverage the test is built with LLVM and left unstripped so kcov can read its DWARF. Coverage counts the sources of libraries in `deps`. The test's own sources count only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it.

kcov runs on Linux only. On macOS and Windows a Zig test is tagged `dx-no-coverage-collector` and writes no coverage. Zig cannot emit `-fprofile-instr-generate`, so LLVM source-based coverage is not available.
