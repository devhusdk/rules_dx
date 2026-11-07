# Use remote execution

Remote execution runs Bazel actions on an executor that speaks the Remote
Execution API (REAPI), such as BuildBuddy, NativeLink or Buildbarn.

## Configure

Put the executor flags in a `remote` config in the repo `.bazelrc`. Commit it.

```text
common:remote --remote_executor=grpcs://executor.example.com
common:remote --remote_timeout=10m
common:remote --jobs=200
common:remote --remote_download_minimal
```

Authenticate with a credential helper that reads the key from the environment, as in
[Use a remote cache](remote-cache.md).

```text
common:remote --credential_helper=remote.example.com=%workspace%/tools/remote-helper.sh
```

Pass the config after `--`.

```sh
dx test //... -- --config=remote
```

`--remote_executor` also serves as the cache, so you need no `--remote_cache`.

## Platforms

Bazel sends each action to an executor that matches its execution platform.
Declare one that carries your executor's properties, and register it.

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

```text
common:remote --extra_execution_platforms=//platforms:linux_x86_64_remote
common:remote --host_platform=//platforms:linux_x86_64_remote
```

The property names, such as `container-image` or `OSFamily`, are defined by
your executor. Check its documentation.

## What the executor needs

- The executor platform must be one `dx` has pinned tools for: Linux x86_64 or
  arm64. See the [support matrix](support-matrix.md).
- Tools are downloaded by Bazel and passed to the executor as action inputs.
- Rules that run a host toolchain (compilers, interpreters) find it through
  Bazel toolchains, not the image `PATH`. Keep the image minimal.
- Quality actions (`dx lint`, `dx typecheck`, `dx format`) are marked
  `no-remote-exec` and run on the machine that started Bazel. Only `shfmt`
  formatting is qualified for remote execution.
- Tests marked `no-remote` or `local` run locally.

## Local slots

When the Bazel config sets `--remote_executor`, `dx` offers more build slots,
because the executor does the heavy work. See
[Build slots](cli/commands/README.md#build-slots).

## Check the setup

Run a small target and look at the process summary.

```sh
dx build //pkg:all -- --config=remote
```

The summary should list `remote` for the actions you expect. If it lists
`linux-sandbox` or `local`, the action did not match the executor platform.
