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
EOF
chmod +x "$work/bin/bazel"
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
