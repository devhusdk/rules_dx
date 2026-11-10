# Use your own C/C++ toolchain

rules_dx registers its own LLVM and GCC toolchains. A toolchain you register yourself takes precedence, and `dx init` treats it as advisory, not a conflict.

## Any ordinary cc_toolchain

Register it in your root `MODULE.bazel`. Nothing from rules_dx is needed. This works for rules_cc `cc_configure`, `toolchains_llvm` or a hand-written `cc_toolchain`:

```python
cc_configure = use_extension("@rules_cc//cc:extensions.bzl", "cc_configure_extension")
use_repo(cc_configure, "local_config_cc", "local_config_cc_toolchains")
register_toolchains("@local_config_cc_toolchains//:all")
```

## A GCC distribution from your own url

For a custom url and sha256 of a GCC build, use the upstream `gcc_toolchain` module. Linux only.

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

Use the `gcc_toolchain` version rules_dx pins in its `MODULE.bazel`.

## Which toolchain wins

Bazel picks the first matching toolchain in this order:

1. `--extra_toolchains` on the command line.
2. `register_toolchains` in the root module, first one wins.
3. The registrations rules_dx adds as a dependency.

`--@rules_dx//cc:compiler` only switches between rules_dx's own toolchains. It does not select yours. See [Bazel toolchains](https://bazel.build/extending/toolchains).

## Private download sources

Use a Bazel credential helper:

```text
common --credential_helper=<host>=<path to helper>
```

`~/.netrc` also works, and `gcc_toolchain` reads it. Keep secrets out of `MODULE.bazel`.

## GCC-specific behaviour

Under an external GCC toolchain, `cc_test` sets `ASAN_OPTIONS=verify_asan_link_order=0` from the resolved toolchain, not from the dx flag. Coverage features follow `--compiler=<name>`, because a transition cannot see the resolved toolchain. Pass `--compiler=gcc` (or `--@rules_dx//cc:compiler=gcc`) with `--collect_code_coverage` under GCC. Without it the build fails at analysis with a message naming the flag, instead of compiling with LLVM coverage features.

## Support

A custom toolchain is your configuration. It is not a qualified rules_dx backend. Its correctness and licences are yours.
