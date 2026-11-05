#!/bin/sh
set -eu
xcrun simctl list runtimes available
sw_vers
xcodebuild -version
DX_SIMCTL_DEVICE=$(xcrun simctl list devices available | sed -n "s/^ *$DX_SIMCTL_DEVICE (\([0-9A-F-]*\)) .*/\1/p" | head -n 1)
export DX_SIMCTL_DEVICE
echo "simulator $DX_SIMCTL_DEVICE"
xcrun simctl list devices available | grep "$DX_SIMCTL_DEVICE"
while [ "$#" -gt 0 ]; do
  DX_SIMCTL_OP="$1" "$2"
  shift 2
done
