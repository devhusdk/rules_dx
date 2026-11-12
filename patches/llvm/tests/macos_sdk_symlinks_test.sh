#!/bin/sh
# The macOS SDK repo holds copies of header files, not file symlinks, and flat frameworks without Versions or symlinks.
set -eu
sysroot=$(readlink -f "$1")
frameworks="$sysroot/System/Library/Frameworks"
for file in \
    "$sysroot/usr/include/pthread.h" \
    "$frameworks/CoreServices.framework/Frameworks/CarbonCore.framework/Headers/Threads.h" \
    "$frameworks/Foundation.framework/Headers/Foundation.h" \
    "$frameworks/Foundation.framework/Foundation.tbd"; do
    if [ ! -f "$file" ] || [ -L "$file" ]; then
        echo "$file is not a regular file" >&2
        exit 1
    fi
done
links=$(find "$sysroot" -type l ! -xtype d)
if [ -n "$links" ]; then
    echo "$links" >&2
    echo "file symlinks remain in $sysroot" >&2
    exit 1
fi
links=$(find "$frameworks" -type l)
if [ -n "$links" ]; then
    echo "$links" >&2
    echo "symlinks remain in $frameworks" >&2
    exit 1
fi
versions=$(find "$frameworks" -type d -path '*.framework/Versions')
if [ -n "$versions" ]; then
    echo "$versions" >&2
    echo "Versions directories remain in $frameworks" >&2
    exit 1
fi
