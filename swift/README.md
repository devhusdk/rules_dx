# Swift

Build Swift for Linux with `rules_swift` 4.2.2 and a pinned Swift 6.4.1
toolchain. Nothing needs to be installed. `dx` registers the toolchain for
Linux x86_64 and aarch64 hosts.

```python
load("@rules_dx//swift/rules:defs.bzl", "swift_binary", "swift_library")

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
You need no `--platforms` or `--extra_execution_platforms` flags.
Binaries and tests link the Swift runtime statically and need only the system C library.

```sh
dx build //app:hello
dx test //app:tests
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

## macOS arm64

On macOS the same rules build with the Xcode installed on the host. Install
Xcode first; no Swift toolchain is downloaded. Swift targets use the Xcode
C/C++ toolchain automatically, so `MODULE.bazel` needs no edits. Binaries link
the system Swift runtime dynamically.

```sh
dx build //app:hello
dx test //app:tests
```

## Toolchain

The sections below describe the Linux toolchain.

- The compiler is the Swift 6.4.1 Ubuntu 22.04 archive from swift.org.
- Headers and C libraries come from a locked Ubuntu 22.04 package set.
- Libraries `swiftc` needs on the build host, such as `libncurses`, come from the same package set.
- C++ resolution stays with `@llvm`.

The build host needs glibc 2.35 or newer.
