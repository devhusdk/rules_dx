# Swift

This directory helps you build, run and test Swift code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//swift:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Swift page](../docs/languages/swift.md) shows
setup, every rule and the tools dx runs.

Run the Swift tests from a rules_dx checkout:

```sh
tools/dx test //swift/...
```

## Quality

### Format and lint

`swift_format` formats and `swift_lint` lints Swift sources, both with
`swift-format` from the pinned toolchain and its default style. They are off
by default. Turn them on in `dx.toml`:

```toml
[families.swift]
format = ["+swift_format"]
lint = ["+swift_lint"]
```

Then run them:

```sh
dx format --apply
dx lint
```

### Typecheck

`swiftc` typechecks Swift. It is off by default. Turn it on in `dx.toml`:

```toml
[families.swift]
typecheck = ["+swiftc"]
```

`dx typecheck` then replays the `SwiftCompile` action of each
`swift_library`, `swift_binary` and `swift_test` with `-typecheck`. It keeps
the module, search paths and `-D` flags and drops every output flag. It runs
the compiler the rule already uses and downloads nothing. Diagnostics point at
their source position. A `swiftc` failure without a parsed error fails the
run. Params files are rejected.

## Bench and fuzz

`swift_fuzz_test` builds a libFuzzer binary with AddressSanitizer and defines
`<name>_run` for `dx fuzz`. It runs on Linux only.

```python
load("@rules_dx//swift:defs.bzl", "swift_fuzz_test")

swift_fuzz_test(
    name = "parse_fuzz",
    srcs = ["parse.swift"],
)
```

Export `LLVMFuzzerTestOneInput` from the source with `@_cdecl`. The test
links the libFuzzer from the Swift toolchain, because the pinned LLVM's
libFuzzer needs libc++ and Swift binaries link libstdc++.

`swift_bench` builds a `swift_binary` that times a loop with `dxBench` and
writes `dx-bench.json` for `dx bench`. It downloads no package-benchmark.
Put top-level code in a `main.swift`:

```python
load("@rules_dx//swift:defs.bzl", "swift_bench")

swift_bench(
    name = "parse_bench",
    srcs = ["main.swift"],
)
```

## API docs

`dx docs` documents every `swift_library` in scope. It reads the symbol graph
that `swift-symbolgraph-extract` from the Swift toolchain writes for the
public declarations, and the library's own `.swift` files. You install
nothing. To declare a docs target yourself, use `swift_doc_shard` from
`@rules_dx//swift:defs.bzl`. Members that an extension adds to a
type from another module are not documented.

## Toolchain

### Writing targets

`swift_library`, `swift_binary` and `swift_test` build for Linux with no
`--platforms` or `--extra_execution_platforms` flags. To build for another
platform, pass `platforms = ["//your:platform"]` to the rule. It takes one
label. Leave it empty to use the current target platform.

### Linux

- The compiler is the Swift 6.4.2 Ubuntu 22.04 archive from swift.org.
- Headers and C libraries come from a locked Ubuntu 22.04 package set.
- Libraries `swiftc` needs on the build host, such as `libncurses`, come from
  the same package set.
- C++ resolution stays with `@llvm`.

The build host needs glibc 2.35 or newer. The Swift tools run on the host CPU
and build for `aarch64` and `x86_64` Linux alike. Binaries and tests link the
Swift runtime statically and need only the system C library.

### macOS arm64

On macOS, the same rules build with the Xcode installed on the host. Install
Xcode first; no Swift toolchain is downloaded. Swift targets use the Xcode
C/C++ toolchain on their own, so your `MODULE.bazel` needs no edits. Binaries
link the system Swift runtime dynamically.

```sh
dx build //app:hello
dx test //app:tests
```

### Windows x86_64

On a Windows x86_64 host, the rules build with the Swift 6.4.2 Windows
installer from swift.org, pinned by sha256. The first Swift build downloads
the 2 GB installer and unpacks it with the pinned PowerShell and 7-Zip.
Nothing is installed on the host and no host program runs. The toolchain is
the `Asserts` one from the installer. The fetch fails unless `swiftc.exe` and
`swiftCore.dll` are PE images for the host architecture.

Swift targets link with MSVC, so accept the Visual Studio Build Tools
license, as the [C and C++ README](https://github.com/devhusdk/rules_dx/blob/main/cc/README.md#msvc) shows. Binaries link the Swift runtime dynamically and
always link the dynamic CRT, also as `rust_test` data. The Swift runtime,
XCTest and Swift Testing DLLs sit next to each executable, so Swift binaries
and tests run as `sh_test` or `rust_test` data.

### Windows arm64

A Windows arm64 host uses the Swift 6.4.2 `windows10-arm64` installer, pinned
by sha256 and unpacked the same way.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
