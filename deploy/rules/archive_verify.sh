#!/usr/bin/env bash
set -euo pipefail

tarball="$(realpath "$1")"
checksum="$(realpath "$2")"
member="$3"

members="$(tar -tzf "${tarball}")"
echo "${members}" | grep -qx "${member}" || {
  echo "archive member '${member}' not found in ${tarball}" >&2
  echo "tarball contents:" >&2
  echo "${members}" >&2
  exit 1
}

expected="$(cut -d' ' -f1 "${checksum}")"
actual="$(sha256sum "${tarball}" | cut -d' ' -f1)"
[[ "${expected}" == "${actual}" ]] || {
  echo "checksum mismatch for ${tarball}" >&2
  echo "  expected: ${expected}" >&2
  echo "  actual:   ${actual}" >&2
  exit 1
}
echo "archive OK: ${tarball} holds ${member}, sha256 ${actual}"
