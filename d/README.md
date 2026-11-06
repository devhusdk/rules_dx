# D

Build, run and test D with the pinned `rules_d` 0.12.1 and LDC 1.43.0.
Nothing needs to be installed. rules_dx registers the toolchain, so there is
no setup in `MODULE.bazel`. Linking uses the hermetic LLVM C++ toolchain.

## Start

Run `dx init --apply demo-app` and add sources. The language is wired when
its sources appear.

```python
load("@rules_dx//d/rules:defs.bzl", "D_COMPATIBLE_WITH", "d_binary", "d_library", "d_test")

d_library(
    name = "greet",
    srcs = ["greet.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
)

d_binary(
    name = "hello",
    srcs = ["main.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
    deps = [":greet"],
)

d_test(
    name = "greet_test",
    srcs = ["greet.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
)
```

- `srcs` lists the `.d` and `.di` files the target owns.
- `d_test` compiles `srcs` with `-unittest` and runs every `unittest` block.
- `deps` takes D libraries and `cc_library` targets.
- `imports`, `string_srcs`, `string_imports`, `versions`, `dopts` and
  `linkopts` pass through to `rules_d`. Files in `string_srcs` are read with
  `import("file")` and are not D sources.
- `D_COMPATIBLE_WITH` covers every pinned LDC cell: linux x86_64, linux arm64, macOS arm64,
  Windows x86_64 and Windows arm64. windows_arm64 runs the x64 LDC under emulation. Both Windows
  cells constrain on the `@llvm//constraints/windows/abi:msvc` ABI, because LDC's prebuilt runtime
  is MSVC-ABI.
- On Windows, D stdio uses ISO wide specifiers, matching cc. `//d/toolchain:msvc_iso_stdio` defines the
  stdio functions druntime would otherwise take from `legacy_stdio_definitions.lib`; hermetic-llvm has
  no legacy mode (hermeticbuild/hermetic-llvm#871). D links the dynamic CRT (`-mscrtlib=msvcrt`).

## Generate

`dx generate` writes one `d_library` per directory of D modules. See
`gazelle/d/testdata/modules/README.md` for the module rules.

Next to a `dub.json`, generation reads the import roots (`importPaths`, else `sourcePaths`, else
`source` or `src`) and fails when a package's modules need a different root. With
`"targetType": "executable"`, the package of the root that defines `main` becomes a `d_binary`.

An import resolves to `@dub//<package>` when its first segment is the name of a dependency in
`dub.json` (with `-` read as `_`) and `dub.selections.json` pins it. Set
`# gazelle:d_dub_hub <name>` in a BUILD file to name the hub repo other than `dub`, for example
`dub_packages`. `# gazelle:resolve d <module> <label>` overrides one import.

## Coverage

`bazel coverage` and `dx coverage` build D libraries and tests with `-cov`, run each test under a small launcher, and convert the `.lst` files ldc writes into LCOV. Coverage covers the sources of targets that match `--instrumentation_filter`. The test's own sources count only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it.

## DUB packages

Packages come from a `dub.selections.json` that DUB wrote. Bazel fetches each
package from `code.dlang.org` through its repository cache; DUB never runs at
fetch time and nothing is resolved.

```starlark
dub = use_extension("@rules_dx//d/dub:extensions.bzl", "dub")
dub.lock(
    name = "dub_packages",
    digests = "//:dub.digests.json",
    selections = "//:dub.selections.json",
)
use_repo(dub, "dub_packages")
```

Set `dev = True` on the tag when you use the extension with `dev_dependency = True`.

Each package is the label `@dub_packages//<name>`, for example
`@dub_packages//silly`; put it in the `deps` of a `d_library`, `d_binary` or
`d_test`. The package's own `dependencies` that appear in the selections become
deps. `sourcePaths`, `sourceFiles`, `importPaths` and `stringImportPaths` of its `dub.json` set
the sources and imports; without `sourcePaths` the `source` or `src` directory is
used, and without `stringImportPaths` the `views` directory is used if it exists.
`@dub_packages//:all_packages` groups every package.

Only plain versions are supported: a `path` or `repository` selection fails, and
so does a package that has a `dub.sdl` instead of a `dub.json`.

`dub.digests.json` pins the archives:

```json
{
  "packages": {
    "silly": {
      "version": "1.1.1",
      "sha256": "<sha256 of https://code.dlang.org/packages/silly/1.1.1.zip>"
    }
  }
}
```

A package without a matching entry fails the fetch and prints the entry to commit.

## Platforms

| Host | LDC | Qualified |
| --- | --- | --- |
| Linux x86_64 | yes | yes |
| Linux arm64 | yes | yes |
| macOS arm64 | yes | yes |
| Windows x86_64 | yes | yes |
| Windows arm64 | via emulation | yes |

The Windows archives are `.7z` files that use BCJ2, which Bazel cannot unpack, so they are
extracted with the pinned 7-Zip in `//libs/starlark:seven_zip.bzl`. LDC has no native Windows
arm64 compiler: that cell runs the x64 LDC under emulation with the arm64 runtime libraries of the
LDC addon package, and compiles with `-mtriple=aarch64-windows-msvc`.

The hello D tests (`greet_test`, `hello_output_test`) pass on Windows x86_64 (evidence run
42057198949) and Windows arm64 (evidence run 42083243205). The hello and d-app tests
(`app_output_test`, `base_test`, `cli_test`) pass on both Windows hosts in evidence run
42302173656.
