#!/bin/sh
set -eu
runfiles="${RUNFILES_DIR:-$0.runfiles}"
cd "$runfiles"
queue="$(mktemp "${TEST_TMPDIR:-/tmp}/queue.XXXXXX")"
seen="$(mktemp "${TEST_TMPDIR:-/tmp}/seen.XXXXXX")"
for tool in "$@"; do
    found="$(find . -path "*/$tool" | head -n 1)"
    if [ -z "$found" ]; then
        echo "$tool is not in the runfiles" >&2
        exit 1
    fi
    echo "$found" >>"$queue"
done
find . -path '*/plugins/*' -name '*.dylib' >>"$queue"
status=0
normalize() {
    echo "$1" | awk -F/ '{ n = 0; for (i = 1; i <= NF; i++) { if ($i == "" || $i == ".") continue; if ($i == ".." && n > 0 && part[n] != "..") { n--; continue } part[++n] = $i } out = "."; for (i = 1; i <= n; i++) out = out "/" part[i]; print out }'
}
while [ -s "$queue" ]; do
    image="$(head -n 1 "$queue")"
    sed -i '' 1d "$queue"
    if grep -Fxq "$image" "$seen"; then
        continue
    fi
    echo "$image" >>"$seen"
    dir="$(dirname "$image")"
    id="$(otool -D "$image" | awk 'NR > 1 { print $1 }')"
    rpaths="$(otool -l "$image" | awk '$1 == "cmd" && $2 == "LC_RPATH" { want = 1 } want && $1 == "path" { print $2; want = 0 }')"
    for dep in $(otool -L "$image" | awk 'NR > 1 { print $1 }'); do
        case "$dep" in
        /usr/lib/* | /System/*) continue ;;
        "$id") continue ;;
        @rpath/*)
            resolved=""
            for rpath in $rpaths; do
                case "$rpath" in
                @loader_path* | @executable_path*) base="$dir${rpath#@*_path}" ;;
                *) base="$rpath" ;;
                esac
                if [ -e "$base/${dep#@rpath/}" ]; then
                    resolved="$base/${dep#@rpath/}"
                    break
                fi
            done
            ;;
        @loader_path/* | @executable_path/*)
            resolved="$dir/${dep#@*_path/}"
            [ -e "$resolved" ] || resolved=""
            ;;
        *) resolved="" ;;
        esac
        if [ -z "$resolved" ]; then
            echo "$image loads $dep, which is not in its runfiles" >&2
            status=1
            continue
        fi
        resolved="$(normalize "$resolved")"
        echo "$image -> $resolved"
        echo "$resolved" >>"$queue"
    done
done
exit "$status"
