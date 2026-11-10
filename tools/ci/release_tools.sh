#!/usr/bin/env bash
# Builds the release-hosted quality tools for one host and collects their licences and sources.
# usage: release_tools.sh OUT HOST
# Run by release-tools.yml.
set -euo pipefail
out="$1"
host="$2"
deadnix_version="1.3.1"
selene_version="0.32.0"
selene_sha256="cd208a4b3bae38decc9c7bb797c19615caaecf67606b3e593ca60b64d3416cd5"
rm -rf "$out"
mkdir -p "$out"
flags=()
ext=""
case "$host" in
  linux-*) flags=(--config=dx_tool_release_linux) ;;
  windows-*) ext=".exe" ;;
esac
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  else
    shasum -a 256 -- "$1" | cut -d' ' -f1
  fi
}
bazel test "${flags[@]}" //lua/tests:selene_smoke_test
bazel build "${flags[@]}" //rust/private/quality/tools:deadnix //rust/private/quality/tools:selene
root="$(bazel info "${flags[@]}" execution_root)"
for tool in deadnix selene; do
  case "$tool" in deadnix) version="$deadnix_version" ;; *) version="$selene_version" ;; esac
  built="$root/$(bazel cquery "${flags[@]}" --output=files "//rust/private/quality/tools:$tool")"
  cp "$built" "$out/dx-tool-$tool-$version-$host$ext"
done
case "$host" in
  linux-*)
    for tool in deadnix-$deadnix_version selene-$selene_version; do
      if readelf -l "$out/dx-tool-$tool-$host" | grep -q INTERP; then
        echo "dx-tool-$tool-$host is dynamically linked" >&2
        exit 1
      fi
    done
    ;;
esac
download="$(mktemp -d)"
crate="deadnix-$deadnix_version.crate"
curl --fail --silent --show-error --location -o "$download/$crate" "https://static.crates.io/crates/deadnix/$crate"
want="$(sed -n '/^name = "deadnix"$/,/^$/s/^checksum = "\(.*\)"$/\1/p' rust/private/quality/tools/Cargo.lock)"
test -n "$want" || { echo "no deadnix checksum in Cargo.lock" >&2; exit 1; }
test "$(sha256_of "$download/$crate")" = "$want" || { echo "deadnix crate sha256 differs from Cargo.lock" >&2; exit 1; }
cp "$download/$crate" "$out/dx-tool-deadnix-$deadnix_version-source.crate"
tar -xzOf "$download/$crate" "deadnix-$deadnix_version/LICENSE" > "$out/dx-tool-deadnix-$deadnix_version-LICENSE"
curl --fail --silent --show-error --location -o "$download/selene.tar.gz" "https://github.com/Kampfkarren/selene/archive/refs/tags/$selene_version.tar.gz"
test "$(sha256_of "$download/selene.tar.gz")" = "$selene_sha256" || { echo "selene archive sha256 differs from MODULE.bazel" >&2; exit 1; }
tar -xzOf "$download/selene.tar.gz" "selene-$selene_version/LICENSE.md" > "$out/dx-tool-selene-$selene_version-LICENSE"
test -s "$out/dx-tool-deadnix-$deadnix_version-LICENSE"
test -s "$out/dx-tool-selene-$selene_version-LICENSE"
rm -rf "$download"
