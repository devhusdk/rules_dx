#!/bin/sh
set -eu

tools=$1 runtime=$2 name=$3 version=$4 image=$5 identity=$6 binary=$7 desktop=$8 icon=$9
shift 9
tool_version=$1 runtime_version=$2 squashfs_version=$3
shift 3

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
app="$work/$name.AppDir"
mkdir -p "$app/usr/bin" "$app/usr/lib" "$app/usr/share/$name" "$app/usr/share/licenses/$name"

cp -L "$binary" "$app/usr/bin/$name"
chmod 755 "$app/usr/bin/$name"
cp -L "$desktop" "$app/$name.desktop"
case "$icon" in
*.svg) cp -L "$icon" "$app/$name.svg" ;;
*) cp -L "$icon" "$app/$name.png" ;;
esac
ln -s "usr/bin/$name" "$app/AppRun"

for entry in "$@"; do
  case "$entry" in
  lib:*)
    cp -L "${entry#lib:}" "$app/usr/lib/"
    ;;
  notice:*)
    cp -L "${entry#notice:}" "$app/usr/share/licenses/$name/"
    ;;
  data:*)
    rest=${entry#data:}
    rel=${rest%%:*}
    src=${rest#*:}
    mkdir -p "$app/usr/share/$name/$(dirname "$rel")"
    cp -L "$src" "$app/usr/share/$name/$rel"
    ;;
  esac
done

grep -q "^Exec=$name\( \|$\)" "$app/$name.desktop" || {
  echo "dx_appimage: $name.desktop must have Exec=$name" >&2
  exit 1
}

esc() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

hash() { sha256sum "$1" | cut -d' ' -f1; }

PATH="$tools:$PATH" ARCH=x86_64 "$tools/appimagetool" --runtime-file "$runtime" --no-appstream "$app" "$image" >"$work/log" 2>&1 || {
  cat "$work/log" >&2
  exit 1
}

glibc=$({ grep -aoE 'GLIBC_[0-9]+\.[0-9]+(\.[0-9]+)?' "$binary" || true; find "$app/usr/lib" -type f | while IFS= read -r lib; do grep -aoE 'GLIBC_[0-9]+\.[0-9]+(\.[0-9]+)?' "$lib" || true; done; } | sed 's/GLIBC_//' | sort -uV | tail -n 1 || true)
interp=$(grep -aoE '/lib[0-9]*/ld-linux[-A-Za-z0-9_.]*' "$binary" | sort -u | head -n 1 || true)

{
  printf '{\n'
  printf '  "name": "%s",\n' "$(esc "$name")"
  printf '  "version": "%s",\n' "$(esc "$version")"
  printf '  "arch": "x86_64",\n'
  printf '  "appimagetool": "%s",\n' "$(esc "$tool_version")"
  printf '  "runtime": {"version": "%s", "sha256": "%s"},\n' "$(esc "$runtime_version")" "$(hash "$runtime")"
  printf '  "mksquashfs": "%s",\n' "$(esc "$squashfs_version")"
  printf '  "interpreter": "%s",\n' "$(esc "$interp")"
  printf '  "min_glibc": "%s",\n' "$(esc "$glibc")"
  printf '  "image_sha256": "%s",\n' "$(hash "$image")"
  printf '  "files": {\n'
  first=1
  (cd "$app" && find . -type f | sort) | while IFS= read -r file; do
    path=${file#./}
    if [ "$first" = 1 ]; then first=0; else printf ',\n'; fi
    printf '    "%s": "%s"' "$(esc "$path")" "$(hash "$app/$path")"
  done
  printf '\n  }\n}\n'
} >"$identity.tmp"
mv "$identity.tmp" "$identity"
