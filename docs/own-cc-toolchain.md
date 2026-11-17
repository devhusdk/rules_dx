# Use your own C/C++ toolchain

This page helps you build C and C++ with a compiler you choose instead of the ones rules_dx brings. A toolchain is how Bazel finds a compiler, linker and their settings.

rules_dx registers its own LLVM and GCC toolchains. A toolchain you register yourself wins over them, and `dx init` treats it as advisory, not a conflict.

## Register any cc_toolchain

Register it in your root `MODULE.bazel`, the module file at the top of your repo. You need nothing from rules_dx. This works for rules_cc `cc_configure`, `toolchains_llvm` or a hand-written `cc_toolchain`. For example, to use the compiler installed on your machine:

```python
cc_configure = use_extension("@rules_cc//cc:extensions.bzl", "cc_configure_extension")
use_repo(cc_configure, "local_config_cc", "local_config_cc_toolchains")
register_toolchains("@local_config_cc_toolchains//:all")
```

## Use a GCC build from your own URL

To download a GCC build from your own URL with your own sha256, use the upstream `gcc_toolchain` module. This works on Linux only.

```python
bazel_dep(name = "gcc_toolchain", version = "0.13.0")

gcc_toolchains = use_extension("@gcc_toolchain//toolchain:module_extensions.bzl", "gcc_toolchains")
gcc_toolchains.toolchain(
    name = "my_gcc_x86_64",
    gcc_version = "16.2.0",
    gcc_versions = """{"16.2.0": {"x86_64": {
        "url": "https://example.com/gcc-toolchain-16.2.0-x86_64.tar.xz",
        "sha256": "<sha256 of the archive>"}}}""",
    target_arch = "x86_64",
)
use_repo(gcc_toolchains, "my_gcc_x86_64")

register_toolchains("@my_gcc_x86_64//:all")
```

Use the same `gcc_toolchain` version that rules_dx pins in its `MODULE.bazel`.

### Coverage and sanitizers with GCC

To collect coverage under GCC, pass `--compiler=gcc` (or `--@rules_dx//cc:compiler=gcc`) with `--collect_code_coverage`. Coverage settings follow that flag, not the toolchain Bazel picked. Without it, the build stops with a message that names the flag.

Under your GCC toolchain, `cc_test` sets `ASAN_OPTIONS=verify_asan_link_order=0` for you.

## Which toolchain wins

Bazel picks the first matching toolchain in this order:

1. `--extra_toolchains` on the command line.
2. `register_toolchains` in your root module, first one first.
3. The toolchains rules_dx registers.

`--@rules_dx//cc:compiler` only switches between rules_dx's own toolchains. It does not select yours. See [Bazel toolchains](https://bazel.build/extending/toolchains).

## Private download sources

Use a Bazel credential helper, a script Bazel runs to get the login for a host:

```text
common --credential_helper=<host>=<path to helper>
```

`~/.netrc` also works, and `gcc_toolchain` reads it. Keep secrets out of `MODULE.bazel`.

## Support

A toolchain you bring is your own setup, not a rules_dx backend. You own its correctness and its licences.
