#!/bin/sh
# libunwind.a must be a regular file; a symlink dangles once its target leaves a reused output base (#7781).
set -eu
if [ -L "$1" ]; then
    echo "$1 is a symbolic link" >&2
    exit 1
fi
[ -f "$1" ]
