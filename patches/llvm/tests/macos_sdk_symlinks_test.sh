#!/bin/sh
# The macOS SDK repo holds copies of header files, not file symlinks, and no Versions/Current links.
set -eu
sysroot=$(readlink -f "$1")
if [ ! -f "$sysroot/usr/include/pthread.h" ] || [ -L "$sysroot/usr/include/pthread.h" ]; then
    echo "$sysroot/usr/include/pthread.h is not a regular file" >&2
    exit 1
fi
links=$(find "$sysroot" -type l ! -xtype d)
if [ -n "$links" ]; then
    echo "$links" >&2
    echo "file symlinks remain in $sysroot" >&2
    exit 1
fi
current=$(find "$sysroot" -type l -name Current -path '*/Versions/Current')
if [ -n "$current" ]; then
    echo "$current" >&2
    echo "Versions/Current symlinks remain in $sysroot" >&2
    exit 1
fi
