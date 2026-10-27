# AppImage example

A Linux C app packaged as an AppImage. The app reads a packaged resource and
calls a function from a shared library in another package.

| Package | Role |
| --- | --- |
| `app` | `dxappimage`, `greeting.txt`, desktop file, icon, notice, the `app` image and `run_test` |
| `greet` | `libgreet_shared.so` |

Build on a Linux host:

```sh
bazel build //examples/appimage-app/app
```

Run the image without FUSE:

```text
APPIMAGE_EXTRACT_AND_RUN=1 bazel-bin/examples/appimage-app/app/app.AppImage
bazel-bin/examples/appimage-app/app/app.AppImage --appimage-extract
APPDIR="$PWD/squashfs-root" squashfs-root/AppRun
```

`dxappimage` prints:

```text
resource=hello from the packaged resource
library=greeting from libgreet_shared.so
probe=off
```

Building with `--copt=-DDX_CONSUMER_PROBE` prints `probe=on`.

Exit codes: `0` ok, `2` `APPDIR` is not set, `3` the resource is missing, `4`
the resource is empty. Without `libgreet_shared.so` the loader stops the app
before it starts.

`app.AppImage.identity.json` records the app name and version, arch, minimum
glibc, image digest and a digest for every packaged file.

Test:

```sh
bazel test //examples/appimage-app/...
```

`run_test` copies the image to a new directory and runs it both ways. It checks
every extracted file against the identity file, exit codes `2` and `3`, a
missing library and a truncated image.

Build for arm64:

```sh
bazel build //examples/appimage-app/app --platforms=//appimage/platforms:linux_arm64
```

Supported hosts and run routes: [AppImage supported tuples](https://github.com/devhusdk/rules_dx/blob/main/appimage/README.md#supported-tuples).
