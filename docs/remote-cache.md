# Use a remote cache

A remote cache lets your team and CI reuse each other's build outputs.

## Local and team

Put the cache flags in a `cache` config in the repo `.bazelrc`. Commit it.

```text
common:cache --remote_cache=grpcs://cache.example.com
common:cache --remote_upload_local_results=true
```

Keep the key out of rc files. Point Bazel at a credential helper that reads the key from the environment. Commit the helper, not the key.

```sh
#!/bin/sh
printf '{"headers":{"x-api-key":["%s"]}}\n' "$CACHE_API_KEY"
```

On Windows, use a `.bat` helper.

```bat
@echo {"headers":{"x-api-key":["%CACHE_API_KEY%"]}}
```

Register it for the cache host in the `cache` config.

```text
common:cache --credential_helper=cache.example.com=%workspace%/tools/cache-helper.sh
```

`--remote_header=name=value` in `~/.bazelrc` or `user.bazelrc` also works on a machine only you use. Do not use it in CI or with `--announce_rc`, which print the value into logs.

Pass the config after `--`.

```sh
dx build //... -- --config=cache
```

A read-only key makes Bazel warn that the cache does not accept uploads. Add a config that turns uploads off and use it with that key.

```text
common:cache-ro --config=cache
common:cache-ro --noremote_upload_local_results
```

Or set `DX_REMOTE_CACHE_READ_ONLY=1`. `dx` then adds `--noremote_upload_local_results` to build, test, coverage and run unless the command line sets an upload flag.

`dx` still adds its shared disk cache locally, so Bazel uses both caches. Under `CI=true`, `dx` adds no disk cache.

## Backends

bazel-remote, with basic auth. The helper prints `Authorization: Basic <base64 user:password>`:

```text
common:cache --remote_cache=grpc://cache.example.com:9092
common:cache --credential_helper=cache.example.com=%workspace%/tools/cache-helper.sh
```

BuildBuddy:

```text
common:cache --remote_cache=grpcs://remote.buildbuddy.io
common:cache --credential_helper=remote.buildbuddy.io=%workspace%/tools/buildbuddy-credential-helper.sh
```

The helper prints `x-buildbuddy-api-key` from `BUILDBUDDY_API_KEY`.

Any other REAPI cache takes its URL in `--remote_cache`. Authenticate with `--credential_helper`.

## CI

- GitHub: see [Caching](github-ci.md#caches).
- GitLab: see the [remote cache setup](https://github.com/devhusdk/rules_dx/blob/main/examples/gitlab-ci/README.md#remote-cache).
