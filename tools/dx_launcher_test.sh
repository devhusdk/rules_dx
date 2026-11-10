#!/bin/sh
set -eu
unset BAZEL_TEST CI
dx=$(find -L "${RUNFILES_DIR:-$0.runfiles}" -path '*/tools/dx' -type f | head -n 1)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
ws="$work/my\\bx"
mkdir -p "$ws/tools" "$ws/home" "$work/bin" "$work/cache"
cp "$dx" "$ws/tools/dx"
chmod +x "$ws/tools/dx"
printf '%s\n' 'import %workspace%/cache.bazelrc' >"$ws/.bazelrc"
printf '%s\n' '--nodisk_cache' >"$ws/cache.bazelrc"
cat >"$work/bin/bazel" <<'EOF'
#!/bin/sh
echo "$*" >>"$CALLS"
# The first query writes a partial line without a newline and crashes; the retry writes the whole answer.
case $* in
*query*)
  if [ "$(grep -c query "$CALLS")" = 1 ]; then
    printf '%s' '//cli/cli/b.r'
    exit 37
  fi
  echo '//cli/cli:a.rs'
  ;;
esac
EOF
chmod +x "$work/bin/bazel"
# A dead pid's temp files are swept; a live pid's (this shell's) are kept.
mkdir -p "$ws/.dx/local/launcher"
for f in dx built inputs query; do
  touch "$ws/.dx/local/launcher/$f.999999"
done
touch "$ws/.dx/local/launcher/dx.$$"
cd "$work"
CALLS="$work/calls" PATH="$work/bin:$PATH" HOME="$work/home" XDG_CACHE_HOME="$work/cache" "$ws/tools/dx" >/dev/null 2>&1 || true
calls=$(cat "$work/calls" 2>/dev/null || true)
case $calls in
build\ *) ;;
*)
  echo "the launcher did not run bazel build: $calls" >&2
  exit 1
  ;;
esac
case $calls in
*--disk_cache*)
  echo "the launcher added --disk_cache over the workspace --nodisk_cache: $calls" >&2
  exit 1
  ;;
esac
inputs=$(cat "$ws/.dx/local/launcher/inputs" 2>/dev/null || true)
if [ "$inputs" != '//cli/cli:a.rs' ]; then
  echo "a retried query kept the failed attempt's partial stdout: inputs=$inputs" >&2
  exit 1
fi
for f in dx.999999 built.999999 inputs.999999 query.999999; do
  if [ -e "$ws/.dx/local/launcher/$f" ]; then
    echo "the launcher kept $f from a dead pid" >&2
    exit 1
  fi
done
if [ ! -e "$ws/.dx/local/launcher/dx.$$" ]; then
  echo "the launcher removed dx.$$ from a live pid" >&2
  exit 1
fi
