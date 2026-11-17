# Use a remote cache

This page helps you share build outputs with your team and CI through a remote cache, so nobody rebuilds what someone else already built.

## Set it up

You need three things: a cache URL, a key, and a small script that hands the key to Bazel.

1. Add a `cache` config to the `.bazelrc` file at the root of your repo, and commit it. A config is a named group of flags that Bazel uses only when you ask for it with `--config=<name>`.

   ```text
   common:cache --remote_cache=grpcs://cache.example.com
   common:cache --remote_upload_local_results=true
   common:cache --credential_helper=cache.example.com=%workspace%/tools/cache-helper.sh
   ```

2. Commit the helper script at `tools/cache-helper.sh`. Bazel runs this credential helper before it talks to the cache host, and sends the headers it prints. The script reads the key from the environment, so the key never lands in a file.

   ```sh
   #!/bin/sh
   printf '{"headers":{"x-api-key":["%s"]}}\n' "$CACHE_API_KEY"
   ```

   On Windows, use a `.bat` helper instead.

   ```bat
   @echo {"headers":{"x-api-key":["%CACHE_API_KEY%"]}}
   ```

3. Set `CACHE_API_KEY` and pass the config after `--`.

   ```sh
   dx build //... -- --config=cache
   ```

On a machine only you use, you can put `--remote_header=name=value` in `~/.bazelrc` or `user.bazelrc` instead of a helper. Do not do this in CI or with `--announce_rc`: both print the value into logs.

## Read-only keys

With a read-only key, Bazel warns that the cache does not accept uploads. Add a config that turns uploads off, and use it with that key.

```text
common:cache-ro --config=cache
common:cache-ro --noremote_upload_local_results
```

Or set `DX_REMOTE_CACHE_READ_ONLY=1`. `dx` then adds `--noremote_upload_local_results` to build, test, coverage and run, unless your command line already sets an upload flag.

## The local disk cache

`dx` also adds its own disk cache on your machine, so Bazel uses both caches. `dx` adds no disk cache when `CI=true`, or when any `.bazelrc` line sets `--disk_cache` or `--nodisk_cache`.

## Backends

Any cache that speaks the Remote Execution API (REAPI) works. Put its URL in `--remote_cache` and authenticate with `--credential_helper`.

bazel-remote, with basic auth. The helper prints `Authorization: Basic <base64 user:password>`.

```text
common:cache --remote_cache=grpc://cache.example.com:9092
common:cache --credential_helper=cache.example.com=%workspace%/tools/cache-helper.sh
```

BuildBuddy. The helper prints `x-buildbuddy-api-key` from `BUILDBUDDY_API_KEY`.

```text
common:cache --remote_cache=grpcs://remote.buildbuddy.io
common:cache --credential_helper=remote.buildbuddy.io=%workspace%/tools/buildbuddy-credential-helper.sh
```

## CI

- GitHub: see [Caches](github-ci.md#caches).
- GitLab: see the [remote cache setup](https://github.com/devhusdk/rules_dx/blob/main/examples/gitlab-ci/README.md#remote-cache).
