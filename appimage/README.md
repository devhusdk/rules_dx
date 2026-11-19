# AppImage

This directory helps you package a Linux binary as an AppImage with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//appimage:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [AppImage page](../docs/languages/appimage.md) shows
setup, every rule and the tools dx runs.

Run the AppImage tests from a rules_dx checkout:

```sh
tools/dx test //appimage/...
```

## Packaging

The tools are pinned and fetched by Bazel. You need no FUSE or distro
packages to build or run an AppImage.

### Use

Your `MODULE.bazel` needs nothing extra. The tools come from
`@dx_appimage_tools`, which rules_dx declares and Bazel fetches on first
use. Declare the package in a `BUILD.bazel` file:

```python
load("@rules_dx//appimage:defs.bzl", "appimage_package")

appimage_package(
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

- `binary`: the executable, packaged as built for the target platform.
- `app_name`: defaults to the target name. `desktop` must be `<app_name>.desktop`
  with `Exec=<app_name>`. `icon` must be `<app_name>.png` or `.svg`.
- `libs`: shared libraries copied to `usr/lib`. Link the binary with
  `-Wl,-rpath,$ORIGIN/../lib`.
- `data`: files copied to `usr/share/<app_name>`. Files keep their path relative to
  their data target's package. Files a data target collects from another package
  or repo keep only their file name.
- `notice`: license and NOTICE files copied to `usr/share/licenses/<app_name>`.
- `version`: recorded in the identity file.

Two different `libs`, `notice` or `data` files that map to the same path
fail analysis, the step where Bazel reads BUILD files before it runs
anything. The same file listed twice is packaged once.

Outputs:

- `<name>.AppImage`
- `<name>.AppImage.identity.json`: app name and version, tool and runtime
  versions, arch (`x86_64` or `aarch64`), runtime digest, interpreter, minimum
  glibc, image digest and a digest for every packaged file.

Inside the image, `AppRun` is a link to `usr/bin/<app_name>`. The
runtime sets `$APPDIR`.

`dx init` and `tools/dx generate` write a `<name>_appimage` target with
`version = "1.0"` for you when a directory holds:

- `<name>.desktop`;
- `<name>.png` or `<name>.svg`;
- a `NOTICE`, `LICENSE` or `COPYING` file;
- exactly one `*_binary` target.

### Build and run

```sh
bazel build //pkg:app
```

Building signs nothing, uploads nothing and needs no credentials.

Run the image without FUSE. Either let it extract itself on each run:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 bazel-bin/pkg/app.AppImage
```

Or extract it once and run `AppRun`:

```sh
bazel-bin/pkg/app.AppImage --appimage-extract
APPDIR="$PWD/squashfs-root" squashfs-root/AppRun
```

#### Supported tuples

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
bazel build //pkg:app --platforms=@rules_dx//appimage:linux_arm64
```

Or set `platform = "@rules_dx//appimage:linux_arm64"` on the target.
Unset uses the current target platform.

The image then holds the arm64 build of the binary. Running it needs an
arm64 Linux host.

### Test

Run the AppImage tests from a rules_dx checkout:

```sh
bazel test //appimage/... //appimage/tests/fixtures/appimage_app/...
```

The [appimage_app fixture](tests/fixtures/appimage_app/README.md) is a two-package
app that is built, relocated and run without FUSE in its tests.

## Toolchain

### Pins

| Tool | License |
| --- | --- |
| appimagetool | MIT |
| type 2 runtime (x86_64, aarch64) | MIT |
| mksquashfs | GPL-2.0-or-later |

Versions are in `pins.json`.

## Platforms

AppImages build for `linux_x86_64` and `linux_arm64`. Any other target
platform fails analysis with `target platform is unsupported`.
