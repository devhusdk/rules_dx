# MSIX

This directory helps you package a Windows binary as an MSIX with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//msix:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [MSIX page](../docs/languages/msix.md) shows
setup, every rule and the tools dx runs.

Run the MSIX tests from a rules_dx checkout:

```sh
tools/dx test //msix/...
```

## Packaging

### Use

Declare the package and its signing launcher in a `BUILD.bazel` file:

```python
load("@rules_dx//msix:defs.bzl", "msix_package", "msix_sign")

msix_package(
    name = "app",
    alias = "app.exe",
    binary = ":app_bin",
    data = ["greeting.txt"],
    display_name = "My app",
    identity_name = "Example.App",
    logo = "logo.png",
    notice = ["NOTICE.txt"],
    platform = "@rules_dx//msix:windows_arm64",
    publisher = "CN=Example",
    version = "1.2.3.0",
)

msix_sign(
    name = "app_sign",
    package = ":app",
)
```

`dx init` and `tools/dx generate` write a `<binary>_msix` target for you
when a directory holds:

- a `Package.appxmanifest` whose `Logo` is a PNG path in the directory;
- a `NOTICE`, `LICENSE` or `COPYING` file;
- exactly one `*_binary` target.

The target takes the `Identity` `Name`, `Publisher` and `Version` and the
`DisplayName` from the manifest.

Attributes:

- `platform`: a Windows platform label such as
  `@rules_dx//msix:windows_arm64`. The package and its `binary`, `libs`
  and `data` build for it. Unset uses the current target platform. Do not
  set it with `select()`. `target_compatible_with` only marks
  compatibility; it does not pick the platform.
- `binary`: the `.exe`, packaged at the package root as built for
  `platform`. Every `.dll` in its runfiles, such as its `dynamic_deps`,
  is packaged next to it.
- `identity_name`: package identity name, 3 to 50 letters, digits, dots or dashes.
- `publisher`: distinguished name that starts with `CN=`. It must match the
  signing certificate subject.
- `version`: four parts, each 0 to 65535.
- `logo`: PNG placed at `Assets/<file name>`.
- `notice`: license and NOTICE files placed under `licenses/`. Required.
- `libs`: extra DLLs placed next to the executable, such as plugins.
  Analysis, the step where Bazel reads BUILD files before it runs
  anything, fails for a `libs` entry without a `.dll`, or for a `libs`
  DLL with the same name as a different runfiles or `libs` DLL.
- `data`: files placed under `share/`. Files keep their path relative to their
  data target's package. Files a data target collects from another package or
  repo keep only their file name. The same file listed twice is packaged once.
- `alias`: command-line execution alias such as `app.exe`. Makes the app a
  console app. Empty adds none.
- `application_id`: defaults to `App`.
- `display_name`, `description`, `publisher_display_name`: default to the
  identity name, the display name and the publisher CN, with escapes and
  quotes removed.
- `min_version`, `max_version_tested`: four parts like `version`.
  Default to `10.0.17763.0` and `10.0.26100.0`.

Two files that map to the same package path fail analysis.

Outputs:

- `<name>.msix`: unsigned.
- `<name>.msix.identity.json`: name, publisher, publisher id, version, arch,
  package full name and family name, application id, executable, alias,
  Windows versions, Build Tools version, the PE machine of every `.exe` and
  `.dll`, the package digest and a digest for every packaged file.

Every `.exe` and `.dll` must match the package arch, or the build fails with
`<file> is built for <machine>, but the package targets <arch>`.

### Build

```sh
bazel build //pkg:app
bazel build //pkg:app --platforms=@rules_dx//msix:windows_arm64
```

The `platform` attribute and `--platforms` do the same thing.

The package digest changes between builds. The digests of the packaged
files stay the same.

### Register a package

You install an unsigned package by registering it as a developer. Turn
on Developer Mode, then run in Windows PowerShell:

```text
mkdir $env:TEMP\app
tar -xf bazel-bin\pkg\app.msix -C $env:TEMP\app
Add-AppxPackage -Register $env:TEMP\app\AppxManifest.xml
app.exe
Get-AppxPackage -Name Example.App | Remove-AppxPackage
```

The registration is per user and needs no administrator rights. An x64 host
cannot install or run an arm64 package.

### Sign

`msix_sign` signs a copy of the package with a PFX certificate. Building
it signs nothing; signing happens only on `bazel run`. If the PFX has a
password, set it in `DX_MSIX_PFX_PASSWORD`.

```sh
bazel run //pkg:app_sign -- -Pfx C:\certs\test.pfx -Out C:\out\app.msix
bazel run //pkg:app_sign -- -Pfx C:\certs\test.pfx -Out C:\out\app.msix -Timestamp http://timestamp.example.com
```

For the run, signing adds the certificate to the current user's `My`
store. Afterwards it removes the certificate and its private key, unless
both were already there. An entry that had no key is put back as it was.
The password never goes on the `signtool` command line.

`signtool` runs on an x64 or arm64 host and signs packages of either arch.

Flags:

- `-Pfx`: the certificate and its private key. Its subject must equal the
  package `publisher`.
- `-Out`: where the signed copy goes. Relative paths resolve against the
  directory `bazel run` was started in.
- `-Timestamp`: RFC 3161 timestamp server URL, `http` or `https`. Without it,
  signing makes no network call. With it, `signtool` first signs a copy
  without the server, then signs a fresh copy with the server. The output has
  one timestamped signature.

The launcher checks the certificate before it runs `signtool`. It
prints:

```text
signed=<path>
sha256=<digest of the signed package>
publisher=<package publisher>
signer=<certificate thumbprint>
timestamp=<server URL or none>
```

Exit codes:

- `0` signed.
- `1` `signtool` failed while signing without the timestamp server, such as
  for a certificate without the code signing usage.
- `2` missing `-Pfx` or `-Out`, a certificate file that does not exist, or a
  `-Timestamp` that is not an `http` or `https` URL.
- `3` the certificate cannot be opened, such as for a wrong password, has no
  private key, is not valid now, or its subject does not match the publisher,
  or the package publisher is not a valid distinguished name.
- `4` signing with the timestamp server failed: the server rejected the
  request, sent an invalid response, or could not be reached.

On any failure, `-Out` is not written.

Make a test certificate:

```text
New-SelfSignedCertificate -Type Custom -Subject "CN=Example" -KeyUsage DigitalSignature -CertStoreLocation Cert:\CurrentUser\My -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
```

Export it with `Export-PfxCertificate`. Windows installs a signed package
only when its certificate is trusted in `Cert:\LocalMachine\TrustedPeople`,
which needs administrator rights:

```text
Import-Certificate -FilePath C:\certs\test.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople
Add-AppxPackage -Path C:\out\app.msix
```

Otherwise `Add-AppxPackage` fails with `0x800B0109` and installs nothing.

## Toolchain

You need no Windows SDK or Visual Studio install.

### Pins

| Tool | License |
| --- | --- |
| Microsoft.Windows.SDK.BuildTools (NuGet) | [Windows SDK license](https://aka.ms/WinSDKLicenseURL) |

The version is in `pins.json`.

The package has `makeappx.exe` and `signtool.exe` for x64 and arm64 hosts.
Its SHA-256 is in `pins.json` too.

## Platforms

MSIX packages build for `windows_x86_64` and `windows_arm64` targets, on a
Windows x64 or arm64 host. `msix_package` and `msix_sign` add
`@platforms//os:windows` to `target_compatible_with`, so `bazel build //...`
skips them on Linux and macOS.

- A Windows target with another CPU fails analysis with
  `target platform is unsupported`.
- A build with no Windows execution platform, the platform that runs the
  build tools, fails analysis with
  `packaging needs a windows_x86_64 or windows_arm64 execution platform`.

## License

Accept the Build Tools license in your root `MODULE.bazel`. Until you do,
nothing is downloaded and packaging fails with
`the Windows SDK Build Tools license (https://aka.ms/WinSDKLicenseURL) is not accepted`.

```python
msix = use_extension("@rules_dx//msix:extensions.bzl", "msix")
dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")
dx_licenses.accept(ids = ["windows-sdk-buildtools"])
```

Only the root module's acceptance counts. An unknown license name fails the
extension.

Accepting the license here does not grant you one. You still need your own from Microsoft.
