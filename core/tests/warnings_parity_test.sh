#!/usr/bin/env bash
set -euo pipefail

mapped="cc/rules/defs.bzl java/private/kwargs.bzl scala/private/validate.bzl kotlin/private/defs.bzl csharp/private/kwargs.bzl fsharp/private/kwargs.bzl haskell/private/kwargs.bzl elixir/private/kwargs.bzl ocaml/private/defs.bzl rust/private/defs.bzl d/private/kwargs.bzl fortran/private/kwargs.bzl swift/private/defs.bzl"
opted_out="go/rules/defs.bzl zig/private/defs.bzl typescript/private/defs.bzl dart/private/defs.bzl erlang/private/kwargs.bzl"

root="${TEST_SRCDIR}/${TEST_WORKSPACE}"
status=0
for file in $mapped; do
  if ! grep -qE 'dx_warnings_as_errors\(|dx_warnings_flags\(' "${root}/${file}"; then
    echo "${file}: no warnings_as_errors mapping" >&2
    status=1
  fi
done
for file in $opted_out; do
  if ! grep -q 'dx_without_warnings_attr(' "${root}/${file}"; then
    echo "${file}: neither mapped nor an explicit opt-out" >&2
    status=1
  fi
done
exit "${status}"
