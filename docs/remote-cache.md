# Use a remote cache

A remote cache lets your team and CI reuse each other's build outputs.

## Local and team

Put the cache flags in a `cache` config in the repo `.bazelrc`. Commit it.

```text
common:cache --remote_cache=grpcs://cache.example.com
common:cache --remote_upload_local_results=true
```

Put secrets in `~/.bazelrc` or `user.bazelrc`. Never commit them. `dx` reads `~/.bazelrc`.

```text
common:cache --remote_header=x-api-key=<token>
```

Pass the config after `--`.

```sh
dx build //... -- --config=cache
```

`dx` still adds its shared disk cache locally, so Bazel uses both caches. Under `CI=true`, `dx` adds no disk cache.

## Backends

bazel-remote, with basic auth:

```text
common:cache --remote_cache=grpc://cache.example.com:9092
common:cache --remote_header=Authorization=Basic <base64 user:password>
```

BuildBuddy:

```text
common:cache --remote_cache=grpcs://remote.buildbuddy.io
common:cache --remote_header=x-buildbuddy-api-key=<key>
```

Any other REAPI cache takes its URL in `--remote_cache`. Authenticate with `--remote_header`, or with `--credential_helper` for a helper program.

## CI

- GitHub: see [Caching](github-ci.md#caches).
- GitLab: see the [remote cache setup](https://github.com/devhusdk/rules_dx/blob/main/examples/gitlab-ci/README.md#remote-cache).
