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

code=0
out=$(env -i APPDIR="$root" "$root/AppRun") || code=$?
[ "$code" = 0 ] || fail "extracted AppRun exited $code"
[ "$out" = "$want" ] || fail "extracted AppRun printed '$out', want '$want'"

code=0
env -i "$root/AppRun" >/dev/null 2>&1 || code=$?
[ "$code" = 2 ] || fail "AppRun without APPDIR exited $code, want 2"

greeting_bak="$TEST_TMPDIR/greeting.txt"
cp "$root/usr/share/dxappimage/greeting.txt" "$greeting_bak"
rm "$root/usr/share/dxappimage/greeting.txt"
code=0
env -i APPDIR="$root" "$root/AppRun" >/dev/null 2>&1 || code=$?
[ "$code" = 3 ] || fail "AppRun without the resource exited $code, want 3"

cp "$greeting_bak" "$root/usr/share/dxappimage/greeting.txt"
rm "$root/usr/lib/libgreet_shared.so"
code=0
err=$(env -i APPDIR="$root" "$root/AppRun" 2>&1 >/dev/null) || code=$?
[ "$code" = 127 ] || fail "AppRun without libgreet_shared.so exited $code, want 127"
case "$err" in
  *libgreet_shared.so*) ;;
  *) fail "AppRun without libgreet_shared.so did not name it on stderr: $err" ;;
esac

size=$(wc -c < "$relocated")
head -c $((size / 2)) "$relocated" > "$work/truncated.AppImage"
chmod +x "$work/truncated.AppImage"
code=0
env -i TMPDIR="$TEST_TMPDIR" APPIMAGE_EXTRACT_AND_RUN=1 "$work/truncated.AppImage" >/dev/null 2>&1 || code=$?
[ "$code" != 0 ] || fail "truncated image exited 0"

echo "ok: $files files match identity, arch $(uname -m), probe=$expected, truncated image exited $code"
