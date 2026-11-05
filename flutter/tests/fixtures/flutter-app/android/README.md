
`android:install`, `android:start` and `android:log` target `emulator-5554` with the x64 APK. `android:interaction_test` installs the APK, taps the add button with `adb shell input` and checks the count through `uiautomator dump`. It is tagged `linux-kvm` and `manual`, and it fails when no emulator or no `adb` is present.

The evidence workflow with `android=true` boots the emulator on a `linux_x86_64` runner. The host, KVM, emulator and system image versions are written by that run to `android-versions.txt` in its artifacts, with the last recorded values below.

| Item | Value |
| --- | --- |
| Host | ubuntu runner, linux_x86_64 |
| KVM | enabled through the udev rule in `evidence.yml` |
| System image | `system-images;android-34;google_apis;x86_64` |
| Emulator | latest from sdkmanager at run time |
