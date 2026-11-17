#!/bin/sh
set -eu

image=""
identity=""
expected=""
for arg in "$@"; do
  case "$arg" in
    *.AppImage) image="$arg" ;;
    *.AppImage.identity.json) identity="$arg" ;;
    *) expected="$arg" ;;
  esac
done

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

[ -n "$image" ] || fail "no .AppImage argument"
[ -n "$identity" ] || fail "no identity.json argument"
[ -n "$expected" ] || fail "no expected probe argument"

work="$TEST_TMPDIR/relocated"
mkdir -p "$work"
cp "$image" "$work/dxappimage.AppImage"
chmod +x "$work/dxappimage.AppImage"
relocated="$work/dxappimage.AppImage"

want="resource=hello from the packaged resource
library=greeting from libgreet_shared.so
probe=$expected"
code=0
out=$(env -i TMPDIR="$TEST_TMPDIR" APPIMAGE_EXTRACT_AND_RUN=1 "$relocated") || code=$?
[ "$code" = 0 ] || fail "run exited $code"
[ "$out" = "$want" ] || fail "run printed '$out', want '$want'"

extract="$TEST_TMPDIR/extract"
mkdir -p "$extract"
(cd "$extract" && env -i TMPDIR="$TEST_TMPDIR" "$relocated" --appimage-extract >/dev/null)
root="$extract/squashfs-root"
[ -d "$root" ] || fail "--appimage-extract made no squashfs-root"

files=0
sed -n '/^  "files": {$/,/^  }/p' "$identity" | grep '^    "' > "$TEST_TMPDIR/files.txt" || true
while IFS= read -r line; do
  path=$(printf '%s\n' "$line" | sed 's/^    "\([^"]*\)": "\([0-9a-f]*\)",\{0,1\}$/\1/')
  hash=$(printf '%s\n' "$line" | sed 's/^    "\([^"]*\)": "\([0-9a-f]*\)",\{0,1\}$/\2/')
  [ -f "$root/$path" ] || fail "identity names $path, not in the image"
  got=$(sha256sum "$root/$path" | cut -d' ' -f1)
  [ "$got" = "$hash" ] || fail "$path is $got, identity says $hash"
  files=$((files + 1))
done < "$TEST_TMPDIR/files.txt"
[ "$files" -gt 0 ] || fail "identity lists no files"
grep -q '^    "usr/lib/libgreet_shared.so": ' "$TEST_TMPDIR/files.txt" || fail "identity lacks usr/lib/libgreet_shared.so"
grep -q '^    "usr/share/dxappimage/greeting.txt": ' "$TEST_TMPDIR/files.txt" || fail "identity lacks the resource"

field() {
  grep "^  \"$1\": " "$identity" | sed 's/^  "[^"]*": "\([^"]*\)",\{0,1\}$/\1/'
}

image_sha=$(sha256sum "$relocated" | cut -d' ' -f1)
[ "$(field image_sha256)" = "$image_sha" ] || fail "image_sha256 is $(field image_sha256), image is $image_sha"
[ "$(field arch)" = "$(uname -m)" ] || fail "arch is $(field arch), host is $(uname -m)"
[ "$(field name)" = "dxappimage" ] || fail "name is $(field name)"
[ "$(field version)" = "1.2.3" ] || fail "version is $(field version)"

echo "ok: $files files match identity, arch $(uname -m), probe=$expected"
