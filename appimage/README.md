# AppImage

Package a target binary as a Linux AppImage. The tools are pinned and fetched
by Bazel. No FUSE, distro packages or ambient tools are needed to build or run.

Cell: `linux_x86_64`. `linux_arm64` fails analysis with a typed message.

## Pins

| Tool | Version | License |
| --- | --- | --- |
| appimagetool | 1.9.1 | MIT |
| type 2 runtime | 20251108 | MIT |
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
- `data`: files copied to `usr/share/<app_name>`, keeping package-relative paths.
- `notice`: license and NOTICE files copied to `usr/share/licenses/<app_name>`.
- `version`: recorded in the identity file.
- `tools`, `runtime`: replace the pinned tools.

Outputs:

- `<name>.AppImage`
- `<name>.AppImage.identity.json`: app name and version, tool and runtime
  versions, runtime digest, interpreter, minimum glibc, image digest and a
  digest for every packaged file.

`AppRun` is a link to `usr/bin/<app_name>`. The runtime sets `$APPDIR`.

## Build and run

```
bazel build //pkg:app
```

Building signs nothing, uploads nothing and needs no credentials.

Run the image without FUSE:

```
APPIMAGE_EXTRACT_AND_RUN=1 bazel-bin/pkg/app.AppImage
bazel-bin/pkg/app.AppImage --appimage-extract
```

Failures:

- A missing resource or library fails in the app or the loader, not silently.
- A truncated or corrupted image exits non-zero.
- A wrong desktop or icon name fails analysis.
- An arm64 target fails analysis with `target cell linux_arm64 is unsupported`.

## Test

```
bazel test //appimage/...
```
