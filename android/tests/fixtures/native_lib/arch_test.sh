#!/bin/sh
set -eu
readelf="$1"
device_so="$2"
emulator_so="$3"
device_machine="$("$readelf" --file-header "$device_so" | grep 'Machine:')"
emulator_machine="$("$readelf" --file-header "$emulator_so" | grep 'Machine:')"
echo "device: $device_machine"
echo "emulator: $emulator_machine"
case "$device_machine" in
*AArch64*) ;;
*) echo "device library is not AArch64" >&2; exit 1 ;;
esac
case "$emulator_machine" in
*X86-64*) ;;
*) echo "emulator library is not X86-64" >&2; exit 1 ;;
esac
