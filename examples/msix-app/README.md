# MSIX example

A Windows console app packaged as an MSIX. The app reads a packaged resource,
calls a function from a DLL in another package and prints its package identity.

| Package | Role |
| --- | --- |
| `app` | `dxmsix.exe`, `greeting.txt`, logo, notice, the `app` package and the `app_sign` signer |
| `greet` | `greet.dll` |

Build on a Windows host:

```
bazel build //examples/msix-app/app
```

`app` needs the Windows SDK Build Tools license accepted. rules_dx accepts it
for its own builds.

Install and run it with Developer Mode on, in Windows PowerShell:

```
mkdir $env:TEMP\dxmsix
tar -xf bazel-bin\examples\msix-app\app\app.msix -C $env:TEMP\dxmsix
Add-AppxPackage -Register $env:TEMP\dxmsix\AppxManifest.xml
dxmsix.exe
Get-AppxPackage -Name DxMsix.Example | Remove-AppxPackage
```

`dxmsix.exe` prints:

```
resource=hello from the packaged resource
library=greeting from greet.dll
package=DxMsix.Example_1.2.3.0_x64__kg15hasbevky6
probe=off
```

Building with `--copt=-DDX_CONSUMER_PROBE` prints `probe=on`.

Exit codes: `0` ok, `2` the executable path is unknown, `3` the resource is
missing, `4` the resource is empty. Without `greet.dll` Windows stops the app
before it starts.

Sign a copy with a test certificate:

```
bazel run //examples/msix-app/app:app_sign -- -Pfx C:\certs\test.pfx -Out C:\out\dxmsix.msix
```

`//msix/tests` builds, inspects, installs and signs this example.
