# Swift

Build Swift for Linux with `rules_swift` 4.2.2 and a pinned Swift 6.4.1
toolchain. Nothing needs to be installed. `dx` registers the toolchain for
Linux x86_64 and aarch64 hosts.

```python
load("@rules_swift//swift:swift_binary.bzl", "swift_binary")
load("@rules_swift//swift:swift_library.bzl", "swift_library")

swift_library(
    name = "greet",
    srcs = ["greet.swift"],
    features = ["swift.static_stdlib", "-static_link_cpp_runtimes"],
    module_name = "Greet",
)

swift_binary(
    name = "hello",
    srcs = ["main.swift"],
    features = ["swift.static_stdlib", "-static_link_cpp_runtimes"],
    deps = [":greet"],
)
```

## Building

Build Swift code for a Swift platform:

| Platform | Constraints |
| --- | --- |
| `@rules_dx//swift/platforms:linux_x86_64` | `libstdcxx.11.5.0`, `gnu.2.35` |
| `@rules_dx//swift/platforms:linux_aarch64` | `libstdcxx.11.5.0`, `gnu.2.35` |
| `@rules_dx//swift/platforms:exec_linux_x86_64` | `gnu.2.35` |
| `@rules_dx//swift/platforms:exec_linux_aarch64` | `gnu.2.35` |

```sh
dx build --platforms=//swift/platforms:linux_x86_64 //app:hello
```

A binary built this way links the Swift runtime statically and needs only the
system C library.

## Toolchain

- The compiler is the Swift 6.4.1 Ubuntu 22.04 archive from swift.org.
- Headers and C libraries come from a locked Ubuntu 22.04 package set.
- Libraries `swiftc` needs on the build host, such as `libncurses`, come from the same package set.
- C++ resolution stays with `@llvm`.

## Tests and host tools

`swift_test` and binaries built for the host need an `exec_linux_*` platform as both
the target platform and the first execution platform:

```sh
bazel test --platforms=//swift/platforms:exec_linux_x86_64 \
  --extra_execution_platforms=//swift/platforms:exec_linux_x86_64 //app:tests
```

The build host needs glibc 2.35 or newer.
