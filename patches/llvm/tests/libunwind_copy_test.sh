#!/bin/sh
# libunwind.a must be a regular file; a symlink dangles once its target leaves a reused output base (#7781).
set -eu
target=$1
if [ -L "$target" ]; then
    link=$(readlink "$target")
    case $link in
    /*) target=$link ;;
    *) target=$(dirname "$target")/$link ;;
    esac
fi
if [ -L "$target" ]; then
    echo "$1 resolves to a symbolic link" >&2
    exit 1
fi
[ -f "$target" ]
