# MSIX

Package a Windows target binary as an MSIX. `makeappx` and `signtool` come
from the pinned Windows SDK Build Tools, fetched by Bazel. No Windows SDK or
Visual Studio install is needed. The packer is a PowerShell script run by the
pinned `pwsh` from rules_powershell, with an empty `PATH`.

Cells: `windows_x86_64` and `windows_arm64` targets. Packaging runs on a
Windows x64 or arm64 host. `dx_msix` and `dx_msix_sign` add
`@platforms//os:windows` to `target_compatible_with`, so `bazel build //...`
skips them on Linux and macOS. A Windows target with another CPU fails
analysis with `target cell this platform is unsupported`. A build with no
Windows execution platform fails analysis with
`packaging needs a windows_x86_64 or windows_arm64 execution platform`.

## Pins

| Tool | Version | License |
| --- | --- | --- |
| Microsoft.Windows.SDK.BuildTools (NuGet) | 10.0.28000.2705 | [Windows SDK license](https://aka.ms/WinSDKLicenseURL) |

The package has `makeappx.exe` and `signtool.exe` for x64 and arm64 hosts.
Its SHA-256 is pinned in `msix/pins.bzl`.

## License

The Build Tools license must be accepted in the root `MODULE.bazel`. Until it
is, nothing is downloaded and packaging fails with
`the Windows SDK Build Tools license (https://aka.ms/WinSDKLicenseURL) is not accepted`.

```python
msix_tools = use_extension("@rules_dx//msix:repos.bzl", "msix_tools")
msix_tools.license(accept = ["windows-sdk-buildtools"])
```

Only the root module's acceptance counts. An unknown license name fails the
extension.

## Use

```python
load("@rules_dx//msix:defs.bzl", "dx_msix", "dx_msix_sign")

dx_msix(
    name = "app",
    alias = "app.exe",
    binary = ":app_bin",
    data = ["greeting.txt"],
    display_name = "My app",
    identity_name = "Example.App",
    logo = "logo.png",
    notice = ["NOTICE.txt"],
    publisher = "CN=Example",
    version = "1.2.3.0",
)

dx_msix_sign(
    name = "app_sign",
    package = ":app",
)
```

Attributes:

- `binary`: the `.exe`. Its target-configuration bytes are packaged at the package root.
  Every `.dll` in its runfiles, such as its `dynamic_deps`, is packaged next to it.
- `identity_name`: package identity name, 3 to 50 letters, digits, dots or dashes.
- `publisher`: distinguished name that starts with `CN=`. It must match the
  signing certificate subject.
- `version`: four parts, each 0 to 65535.
- `logo`: PNG placed at `Assets/<file name>`.
- `notice`: license and NOTICE files placed under `licenses/`. Required.
- `libs`: extra DLLs placed next to the executable, such as plugins. A `libs` entry without a `.dll` fails analysis.
- `data`: files placed under `share/`, keeping package-relative paths.
- `alias`: command-line execution alias such as `app.exe`. Makes the app a
  console app. Empty adds none.
- `application_id`: defaults to `App`.
- `display_name`, `description`, `publisher_display_name`: default to the
  identity name, the display name and the publisher CN.
- `min_version`, `max_version_tested`: four parts like `version`. Default to `10.0.17763.0` and `10.0.26100.0`.

Two files that map to the same package path fail analysis.

Outputs:

- `<name>.msix`: unsigned.
- `<name>.msix.identity.json`: name, publisher, publisher id, version, arch,
  package full name and family name, application id, executable, alias,
  Windows versions, Build Tools version, the PE machine of every `.exe` and
  `.dll`, the package digest and a digest for every packaged file.

Every `.exe` and `.dll` must match the package arch, or the build fails with
`<file> is built for <machine>, but the package targets <arch>`.

## Build

```
bazel build //pkg:app
bazel build //pkg:app --platforms=@rules_dx//msix/platforms:windows_arm64
```

Building signs nothing, installs nothing, uploads nothing and needs no
credentials. `makeappx` validates the manifest. Its output embeds timestamps,
so the package digest changes between builds. The digests of the packaged
files do not.

## Install

Unsigned packages install only through a developer registration. Turn on
Developer Mode, then, in Windows PowerShell:

```
mkdir $env:TEMP\app
tar -xf bazel-bin\pkg\app.msix -C $env:TEMP\app
Add-AppxPackage -Register $env:TEMP\app\AppxManifest.xml
app.exe
Get-AppxPackage -Name Example.App | Remove-AppxPackage
```

The registration is per user and needs no administrator rights.

## Sign

`dx_msix_sign` signs a copy of the package with a PFX certificate. Set the
password in `DX_MSIX_PFX_PASSWORD` when the PFX has one.

`signtool` is the one that runs on the host, x64 or arm64. It signs packages of
either arch, so an x64 host signs an arm64 package.

```
bazel run //pkg:app_sign -- -Pfx C:\certs\test.pfx -Out C:\out\app.msix
```

It prints `signed=<path>` and `sha256=<digest>`. Relative paths resolve
against the directory `bazel run` was started in.

Exit codes: `0` signed, `1` signing failed, `2`
missing `-Pfx`, missing `-Out` or a certificate file that does not exist.

Make a test certificate:

```
New-SelfSignedCertificate -Type Custom -Subject "CN=Example" -KeyUsage DigitalSignature -CertStoreLocation Cert:\CurrentUser\My -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
```

Export it with `Export-PfxCertificate`. A package signed with a test
certificate installs only after its certificate is trusted on that machine.

## Test

```
bazel test //msix/...
bazel test //msix/tests:install_test
bazel test //tools/testing/hermetic:msix
```

On Linux, `//msix/...` runs the analysis tests and the packer test against
cross-compiled x64 and arm64 binaries and a stand-in `makeappx`. On Windows,
it builds real packages for x64, arm64 and a `DX_CONSUMER_PROBE` build, checks
their zip entries, block map, manifest, digests and PE machines, and signs one
with a throwaway certificate. `install_test` is manual. It needs Developer
Mode, registers each package for the current user, runs it through its alias,
checks the output and removes it.

`//tools/testing/hermetic:msix` is manual and needs network once. It builds a
separate consumer workspace that names rules_dx `my_dx`, accepts the license
in its root `MODULE.bazel`, and builds and tests it cold, warm and offline.
