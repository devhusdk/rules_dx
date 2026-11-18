# Windows

Use this page to set up a Windows host so `dx` and Bazel work. Run the PowerShell
commands as the user who runs Bazel. Commands marked administrator need an
elevated shell.

## Git for Windows

Install Git for Windows. It provides Git Bash, which runs the `dx init` hooks.

```powershell
winget install --id Git.Git -e
```

## Shell

Genrules need the Git bash. A genrule is a Bazel rule that runs a shell command. When `BAZEL_SH` is unset and the first `bash.exe` on
`PATH` is not the Git bash, `dx` sets `BAZEL_SH` to the Git bash next to `git.exe`
on `PATH`. It exits with an error when only the WSL launcher is found. It never
replaces a value you set. Inside `bazel run`, `dx` removes a `BAZEL_SH` that the outer Bazel auto-detected. Plain `bazel` is different: when
`BAZEL_SH` is unset, Bazel takes the first `bash.exe` on `PATH`. With WSL installed, that is `C:\Windows\System32\bash.exe`,
and genrules fail with `Bash/Service/0x8007274c`. Point Bazel at the Git bash for your user:

```powershell
[Environment]::SetEnvironmentVariable('BAZEL_SH', 'C:\Program Files\Git\usr\bin\bash.exe', 'User')
```

Change the path if Git is installed somewhere else. Open a new shell afterwards.
If you start Bazel from WSL through `cmd.exe`, run `wsl --shutdown` first so WSL
picks up the new variable, then check it:

```sh
cmd.exe /c echo %BAZEL_SH%
```

Or set the shell for Bazel only, in `user.bazelrc`:

```text
build:windows "--shell_executable=C:/Program Files/Git/usr/bin/bash.exe"
```

## PowerShell

`tools\dx.cmd` uses PowerShell to download the pinned `dx` release binary from
GitHub and checks its sha256 against `.dx/pins`. It caches the binary in
`%USERPROFILE%\.cache\dx`. If the download fails, it prints the Bazel command to
run instead and exits with an error. PowerShell needs network access to
`github.com`. For proxy and custom CA setup, see
[Corporate proxy and custom CA](../../proxy-and-ca.md).

Windows PowerShell 5.1 ships with Windows. Check it is on `PATH`:

```powershell
powershell -NoProfile -Command '$PSVersionTable.PSVersion'
```

## Symlinks

The consumer preset sets `build:windows --enable_runfiles`. That needs symlinks.
Turn on Developer Mode in Settings, under System, then For developers. Or grant
the "Create symbolic links" right to your user in Local Security Policy, under
User Rights Assignment. Sign out and in again afterwards.

Add this to `.bazelrc` or `user.bazelrc`:

```text
startup:windows --windows_enable_symlinks
```

## Linker

The consumer preset links Rust with rust-lld on Windows. It also sets the host
and execution platform to `//tools/platforms:windows_msvc_host`, so rustc and
the C++ toolchain both use the MSVC ABI.

The MSVC runtime and the Windows SDK need the Visual Studio Build Tools
license accepted in the root `MODULE.bazel`:

```python
dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")
dx_licenses.accept(ids = ["visual-studio-build-tools"])
```

`dx init --apply --accept-license visual-studio-build-tools` writes these lines.
Without them the toolchain fetch fails and names the license id.

## Long paths

Use a short output root. Add this to `.bazelrc`:

```text
startup:windows --output_user_root=C:/bz
```

Turn on long paths in Windows (administrator):

```powershell
Set-ItemProperty -Path HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem -Name LongPathsEnabled -Value 1 -Type DWord
```

Turn on long paths in Git:

```powershell
git config --global core.longpaths true
```

## Defender exclusions

Real-time scanning of the output root and `.dx` slows builds. Exclude both
(administrator):

```powershell
Add-MpPreference -ExclusionPath 'C:\bz'
Add-MpPreference -ExclusionPath 'C:\path\to\your\repo\.dx'
```

Use the path from `--output_user_root` if you chose another one.

## C/C++ toolchain

The default C/C++ toolchain needs no Visual Studio install.
