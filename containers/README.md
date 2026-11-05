# Containers

`rules_oci` and `rules_pkg` with distroless base images pinned by digest.

| Repository | Image |
| --- | --- |
| `@distroless_static` | `gcr.io/distroless/static-debian12` |
| `@distroless_cc` | `gcr.io/distroless/cc-debian12` |
| `@distroless_python` | `gcr.io/distroless/python3-debian12` |
| `@distroless_java` | `gcr.io/distroless/java21-debian12` |
| `@distroless_nodejs` | `gcr.io/distroless/nodejs22-debian12` |

Each pin is the multi-arch index digest. `@<name>_linux_amd64` and `@<name>_linux_arm64` hold the single-platform images.

```sh
bazel build @distroless_static
```

The first build fetches the layers. Later builds use the Bazel cache and need no network.
