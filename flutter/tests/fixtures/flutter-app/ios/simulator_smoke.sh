#!/bin/sh
set -eu
for launcher in "$@"; do
  "$launcher"
done
