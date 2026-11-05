#!/bin/bash
for f in "$@"; do echo "== $f"; od -c "$f" | head -30; done
exit 1
