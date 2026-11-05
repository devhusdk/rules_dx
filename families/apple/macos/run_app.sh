#!/bin/sh
set -eu

if ! command -v ditto >/dev/null 2>&1; then
    echo "apple_macos_app: no ditto on PATH; running the app needs macOS" >&2
    exit 127
fi
if [ -z "${DX_MACOS_APP_ZIP:-}" ]; then
    echo "apple_macos_app: DX_MACOS_APP_ZIP is not set; run the app with bazel run" >&2
    exit 2
fi
runfiles="${RUNFILES_DIR:-$0.runfiles}"
zipped="$runfiles/$DX_MACOS_APP_ZIP"
if [ ! -f "$zipped" ]; then
    echo "apple_macos_app: $DX_MACOS_APP_ZIP is not in the runfiles" >&2
    exit 2
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/dx-macos-app.XXXXXX")
ditto -x -k "$zipped" "$work"
app=$(find "$work" -maxdepth 1 -name '*.app' | head -n 1)
if [ -z "$app" ]; then
    echo "apple_macos_app: the archive holds no .app" >&2
    exit 1
fi
executable=$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")
exec "$app/Contents/MacOS/$executable" "$@"
