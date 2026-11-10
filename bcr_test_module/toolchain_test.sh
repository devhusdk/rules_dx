#!/bin/sh
set -eu
grep -q "^$2\$" "$1"
