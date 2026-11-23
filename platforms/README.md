# Platforms

Set `platforms` on a rule to choose what it builds for. Each entry is a platform name like `linux_x86_64`, or an OS name like `linux`.

When the build is for none of the listed platforms, the target builds for the first entry. If that entry is an OS name, the target builds for the first full name in the list with that OS, or else for this default:

| OS | Default |
| --- | --- |
| `android` | `android_arm64` |
| `ios` | `ios_arm64` |
| `linux` | `linux_x86_64` |
| `macos` | `macos_arm64` |
| `web` | `web_wasm32` |
| `windows` | `windows_x86_64` |
