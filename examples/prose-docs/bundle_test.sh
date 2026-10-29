#!/bin/sh
set -eu
for page in \
  guide/index.rst guide/install.rst shared/glossary.rst \
  handbook/intro.adoc handbook/ops.adoc shared/terms.adoc \
  paper/main.typ paper/common.typ paper/main.tex paper/body.tex; do
  found=$(find -L . -path "*/examples/prose-docs/$page" | head -n 1)
  if [ -z "$found" ]; then
    echo "missing from runfiles: $page" >&2
    exit 1
  fi
done
