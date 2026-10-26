#!/bin/sh
set -eu
adb_ops="$1"
export DX_ADB_SERIAL="emulator-5554"
DX_ADB_OP=install-multiple DX_ADB_APKS="$2" "$adb_ops"
DX_ADB_OP=start DX_ADB_APP=com.example.dxrustapk DX_ADB_ACTIVITY=android.app.NativeActivity "$adb_ops"
tries=0
until DX_ADB_OP=log "$adb_ops" | grep -q "DxRustApk.*lifecycle="; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || { echo "no DxRustApk lifecycle line in logcat"; exit 1; }
    sleep 1
done
