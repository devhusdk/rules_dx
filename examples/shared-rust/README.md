# Shared Rust core example

One dependency-free Rust core drives a native binary, a browser
application, and an Android package from the same source. The core
evaluates arithmetic expressions and reports invalid input as data, so
every output observes identical behavior.

```sh
bazel build //examples/shared-rust/...
bazel test //examples/shared-rust/...
```

`core_test` covers precedence, errors, and the JNI probe. `asset_test`
evaluates the staged asset natively. `browser_test` runs the relocated
app in a real Firefox, including an invalid-input error and a driven
control. `consumer_browser_test` and `consumer_native_test` repeat the
browser and native checks from the independent `shared_consumer`
workspace, which owns its HTML, asset, and server settings while
reusing the public `shared_core` bindgen and `core_bin` launcher.

Qualified on Linux: native build, test, and run; browser build, serve,
and Firefox execution; relocated independent-consumer browser and
native probes. Release evidence uses the optimized build:

```sh
bazel build -c opt //examples/shared-rust:core_bin
```

Pending, not runnable on this host: `core_apk` packaging and emulator
install need an Android SDK (`ANDROID_HOME` names it, `inspect_apk.sh`
in `android/tests/fixtures/rust_apk` shows the artifact checks), and
Apple simulator, device, launch, and signing need macOS with Xcode (no
Apple toolchain is wired in this tree yet).
