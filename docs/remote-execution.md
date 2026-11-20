# Use remote execution

This page helps you run your build and test actions on a remote executor, such as BuildBuddy, NativeLink or Buildbarn, instead of your own machine. An action is one step Bazel runs, like compiling a file or running a test.

## Set it up

1. Add a `remote` config to the `.bazelrc` file at the root of your repo, and commit it. A config is a named group of flags that Bazel uses only when you pass `--config=<name>`.

   ```text
   common:remote --remote_executor=grpcs://executor.example.com
   common:remote --remote_timeout=10m
   common:remote --jobs=200
   common:remote --remote_download_minimal
   common:remote --credential_helper=executor.example.com=%workspace%/tools/remote-helper.sh
   ```

   The executor also serves as the cache, so you need no `--remote_cache`. The credential helper works the same way as in [Use a remote cache](remote-cache.md).

2. Declare an execution platform for the executor and register it (see below).

3. Pass the config after `--`.

   ```sh
   dx test //... -- --config=remote
   ```

## Platforms

An execution platform describes a machine that can run actions. Bazel sends each action to an executor that matches its execution platform, so declare one that carries your executor's properties.

```python
platform(
    name = "linux_x86_64_remote",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
    exec_properties = {
        "container-image": "docker://registry.example.com/dx-executor@sha256:<digest>",
    },
)
```

Register it in the `remote` config.

```text
common:remote --extra_execution_platforms=//platforms:linux_x86_64_remote
common:remote --host_platform=//platforms:linux_x86_64_remote
```

Your executor defines the property names, such as `container-image` or `OSFamily`. Check its documentation.

## Check the setup

Build a small target and read the process summary at the end.

```sh
dx build //pkg:all -- --config=remote
```

The summary should list `remote` for the actions you expect. If it lists `linux-sandbox` or `local`, the action did not match the executor platform.

## What runs where

- The executor must run on a platform `dx` has pinned tools for: Linux x86_64 or arm64. See the [support matrix](support-matrix.md).
- Bazel downloads the tools and sends them to the executor as action inputs. Compilers and interpreters come from Bazel toolchains, not the image `PATH`, so keep the image minimal.
- A `dx lint`, `dx typecheck` or `dx format` action runs remotely only when every tool in it is remote-qualified. Today those are shfmt, shellcheck, keep-sorted, typos, taplo, buildifier, gersemi, jsonnetfmt, stylua, typstyle, yamlfmt, biome, actionlint, zizmor, ruff format and lint, hadolint, checkmake, fortitude, ffmt, cue, jsonnet-lint, opa, xmllint and Vale on Markdown, AsciiDoc and reST. Every other action is tagged `no-remote-exec` and runs on your machine.
- Unix executor images need `/bin/sh`, because actionlint finds shellcheck through a `/bin/sh` shim.
- Unix executor images need `bash` and coreutils in `/bin` or `/usr/bin`, because Vale's AsciiDoc and reST converters are Bazel shell launchers started with `PATH` unset.
- Tests tagged `no-remote` or `local` run on your machine.

## Local slots

When your Bazel config sets `--remote_executor`, `dx` offers more build slots, because the executor does the heavy work. See [Build slots](cli/commands/README.md#build-slots).
