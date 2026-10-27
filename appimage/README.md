# AppImage

Package a target binary as a Linux AppImage. The tools are pinned and fetched
by Bazel. No FUSE or distro packages are needed to build or run. The build
step is a pinned Rust binary run with an empty `PATH`. It uses no tool from
`/bin` or `/usr/bin`.

Cells: `linux_x86_64` and `linux_arm64`. Any other target fails analysis with
`target cell this platform is unsupported`.

## Pins

| Tool | Version | License |
| --- | --- | --- |
| appimagetool | 1.9.1 | MIT |
| type 2 runtime (x86_64, aarch64) | 20251108 | MIT |
| mksquashfs | 4.6.1 | GPL-2.0-or-later |

## Use

`MODULE.bazel` needs nothing extra. The tools come from `@dx_appimage_tools`,
declared by rules_dx and fetched on first use.

```python
load("@rules_dx//appimage:defs.bzl", "dx_appimage")

dx_appimage(
    name = "app",
    app_name = "app",
    binary = ":app_bin",
    data = ["greeting.txt"],
    desktop = "app.desktop",
    icon = "app.png",
    libs = [":native_shared"],
    notice = ["NOTICE.txt"],
    version = "1.2.3",
)
```

Attributes:

- `binary`: the executable. Its target-configuration bytes are packaged.
- `app_name`: defaults to the target name. `desktop` must be `<app_name>.desktop`
  with `Exec=<app_name>`. `icon` must be `<app_name>.png` or `.svg`.
- `libs`: shared libraries copied to `usr/lib`. Link the binary with
  `-Wl,-rpath,$ORIGIN/../lib`.
- `data`: files copied to `usr/share/<app_name>`. Files keep their path relative to
  their data target's package. Files a data target collects from another package
  or repo keep only their file name.
- `notice`: license and NOTICE files copied to `usr/share/licenses/<app_name>`.
- `version`: recorded in the identity file.
- `tools`, `runtime`: replace the pinned tools and runtime. The runtime must match the target arch.
- Two different `libs`, `notice` or `data` files that map to the same path fail analysis. The same file listed twice is packaged once.

Outputs:

- `<name>.AppImage`
- `<name>.AppImage.identity.json`: app name and version, tool and runtime
  versions, arch (`x86_64` or `aarch64`), runtime digest, interpreter, minimum
  glibc, image digest and a digest for every packaged file.

`AppRun` is a link to `usr/bin/<app_name>`. The runtime sets `$APPDIR`.

## Build and run

```sh
bazel build //pkg:app
```

Building signs nothing, uploads nothing and needs no credentials.

Run the image without FUSE:

```text
APPIMAGE_EXTRACT_AND_RUN=1 bazel-bin/pkg/app.AppImage
bazel-bin/pkg/app.AppImage --appimage-extract
APPDIR="$PWD/squashfs-root" squashfs-root/AppRun
```

### Supported tuples

| Build host | Target cell | Run host | Run route | Install | Sign |
| --- | --- | --- | --- | --- | --- |
| Linux x86_64 or arm64 | `linux_x86_64` | Linux x86_64, glibc at least `min_glibc` | FUSE mount, `APPIMAGE_EXTRACT_AND_RUN=1` or `--appimage-extract` | none, copy the file | unsupported, images are unsigned |
| Linux x86_64 or arm64 | `linux_arm64` | Linux arm64, glibc at least `min_glibc` | FUSE mount, `APPIMAGE_EXTRACT_AND_RUN=1` or `--appimage-extract` | none, copy the file | unsupported, images are unsigned |
| macOS or Windows | any | none | none | none | unsupported, the targets are incompatible |

`min_glibc` is in the identity file.

Failures:

- A missing resource or library fails in the app or the loader, not silently.
- A truncated or corrupted image exits non-zero.
- A wrong desktop or icon name fails analysis.
- Two different files that map to the same path in the image fail analysis.
- A target that is not Linux x86_64 or arm64 fails analysis.
- A desktop file without `Exec=<app_name>` fails the build.

Build for arm64 from any Linux host:

```sh
bazel build //pkg:app --platforms=//appimage/platforms:linux_arm64
```

The image is packaged from the arm64 target bytes. Running it needs an arm64
Linux host.

## Test

```sh
bazel test //appimage/... //examples/appimage-app/...
# Consumer qualification (manual; needs network and a native bazelisk):
bazel test //tools/testing/hermetic:appimage
```

[examples/appimage-app](../examples/appimage-app/README.md) is a two-package
app that is built, relocated and run without FUSE in its tests.
