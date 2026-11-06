#!/bin/sh
# The family's unnamed maven hub must stay dev-only so it never reaches a consumer's @maven.
set -eu
grep -q '^maven = use_extension(.*dev_dependency = True)' "$1" || {
    echo "families/jvm/MODULE.bazel: the maven use_extension must set dev_dependency = True" >&2
    exit 1
}
