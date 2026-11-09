# Swift

Build Swift for Linux with `rules_swift` 4.2.2 and a pinned Swift 6.4.1
toolchain. Nothing needs to be installed. `dx` registers the toolchain for
Linux x86_64 and aarch64 hosts.

```python
load("@rules_dx//swift:defs.bzl", "swift_binary", "swift_library")

swift_library(
    name = "greet",
    srcs = ["greet.swift"],
    module_name = "Greet",
)

swift_binary(
    name = "hello",
    srcs = ["main.swift"],
    deps = [":greet"],
)
```

`swift_library`, `swift_binary` and `swift_test` build for Linux on their own.
You need no `--platforms` or `--extra_execution_platforms` flags. To build for another
platform, pass `platforms = ["//your:platform"]` to the wrapper; it takes one label, and
empty uses the current target platform.
Binaries and tests link the Swift runtime statically and need only the system C library.

```sh
dx build //app:hello
dx test //app:tests
```

## Fuzz and bench

`swift_fuzz_test` builds a libFuzzer binary with AddressSanitizer and defines
`<name>_run` for `dx fuzz`. The source exports `LLVMFuzzerTestOneInput` with
`@_cdecl`. It links the libFuzzer from the Swift toolchain archive, because the
pinned LLVM's libFuzzer needs libc++ and Swift binaries link libstdc++. Linux only.

```python
load("@rules_dx//swift:defs.bzl", "swift_fuzz_test")

swift_fuzz_test(
    name = "parse_fuzz",
    srcs = ["parse.swift"],
)
```

`swift_bench` builds a `swift_binary` that times a loop with `dxBench` from
`//swift/bench` and writes `dx-bench.json` for `dx bench`. It needs no
package-benchmark download. The sources need a `main.swift` for top-level code.

```python
load("@rules_dx//swift:defs.bzl", "swift_bench")

swift_bench(
    name = "parse_bench",
    srcs = ["main.swift"],
)
```

## Format and lint

`dx format` and `dx lint` run `swift-format` from the pinned toolchain on Swift
sources. The default policy runs `swift_format` and `swift_lint` with the
default swift-format style. They run on Linux only; other platforms skip them
with a note.

```sh
dx format --apply
dx lint
```

## API docs

`dx docs` documents every `swift_library` in scope from the symbol graph that `swift-symbolgraph-extract` of the Swift toolchain writes for its public declarations. `swift_doc_shard` (`@rules_dx//tools/docs/swift:defs.bzl`) turns the graph and the library's own `.swift` files into the API reference. Nothing needs to be installed. Members that an extension adds to a type of another module are not documented.

## macOS arm64

On macOS the same rules build with the Xcode installed on the host. Install
Xcode first; no Swift toolchain is downloaded. Swift targets use the Xcode
C/C++ toolchain automatically, so `MODULE.bazel` needs no edits. Binaries link
the system Swift runtime dynamically.

```sh
dx build //app:hello
dx test //app:tests
```

## Windows x86_64

On a Windows x86_64 host the rules build with the Swift 6.4.1 Windows installer
from swift.org, pinned by sha256. The first Swift build downloads the 2 GB
installer and unpacks it with the pinned PowerShell, `expand.exe` and
`msiexec /a`; nothing is installed on the host.

Swift targets link with the Windows C/C++ toolchain, so `dx_msvc` with its
license accepted must be configured. Binaries link the Swift runtime
dynamically; `swift_test` puts the runtime DLLs on `PATH`.

This toolchain is not yet validated: `windows.msi` fails to extract with
`msiexec /a` exit 1603, and Swift is not in the supported platforms.

## Windows arm64

A Windows arm64 host uses the Swift 6.4.1 `windows10-arm64` installer, pinned by
sha256 and unpacked the same way. It is registered but unverified, and Swift
stays unsupported on windows_arm64 until it builds on a Windows ARM runner.

## Toolchain

The sections below describe the Linux toolchain.

- The compiler is the Swift 6.4.1 Ubuntu 22.04 archive from swift.org.
- Headers and C libraries come from a locked Ubuntu 22.04 package set.
- Libraries `swiftc` needs on the build host, such as `libncurses`, come from the same package set.
- C++ resolution stays with `@llvm`.

The build host needs glibc 2.35 or newer.

## Typecheck

`dx typecheck` with `typecheck = ["swiftc"]` on the `swift` family replays the
`SwiftCompile` action of each `swift_library`, `swift_binary` and `swift_test`
with `-typecheck`, keeping its module, search path and `-D` flags and dropping
every output flag. Diagnostics are reported at their source position; a
`swiftc` failure without a parsed error fails closed. It runs the compiler the
rule already uses and downloads nothing. Params files are rejected.
