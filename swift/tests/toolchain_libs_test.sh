#!/usr/bin/env bash
# Fails when a Swift toolchain binary needs a shared library that neither the
# toolchain nor the sysroot host_libs provides. This is the static form of
# running ldd with LD_LIBRARY_PATH cleared: the loader would take any other
# library from the host. Only the glibc runtime may come from the host.
set -euo pipefail

objdump="$1"
# The toolchain root is two levels above its usr/bin/llvm-objdump; host_libs is
# where the sysroot's libz.so.1 sits.
toolchain="$(cd "$(dirname "$objdump")/../.." && pwd)"
host_libs="$(cd "$(dirname "$2")" && pwd)"

needed() {
  "$objdump" -p "$1" | awk '$1 == "NEEDED" {print $2}'
}

# Where the binaries' RUNPATHs and the wrapper's LD_LIBRARY_PATH point.
dirs=(
  "$toolchain/usr/lib"
  "$toolchain/usr/lib/swift/linux"
  "$toolchain/usr/lib/swift/host"
  "$toolchain/usr/lib/swift/host/compiler"
  "$host_libs"
)

host_ok='^(ld-linux-.*|libc\.so\.6|libm\.so\.6|libdl\.so\.2|libpthread\.so\.0|librt\.so\.1|libutil\.so\.1|libresolv\.so\.2)$'

resolve() {
  local d
  for d in "${dirs[@]}"; do
    [[ -e "$d/$1" ]] && { echo "$d/$1"; return 0; }
  done
  return 1
}

# Skipped: lldb and its Python bindings are not run by any rule.
checked=0
failed=0
declare -A seen=()
queue=()
for f in "$toolchain"/usr/bin/*; do
  [[ -f "$f" ]] || continue
  f="$(readlink -f "$f")"
  [[ "$(head -c4 "$f" | od -An -c | tr -d ' ')" == '177ELF' ]] || continue
  case "$(basename "$f")" in lldb*|repl_swift) continue ;; esac
  queue+=("$f")
done

while ((${#queue[@]})); do
  f="${queue[0]}"
  queue=("${queue[@]:1}")
  [[ -n "${seen[$f]:-}" ]] && continue
  seen[$f]=1
  checked=$((checked + 1))
  for lib in $(needed "$f"); do
    [[ "$lib" =~ $host_ok ]] && continue
    if path="$(resolve "$lib")"; then
      queue+=("$(readlink -f "$path")")
    else
      echo "FAIL: $(basename "$f") needs $lib, which is outside the toolchain" >&2
      failed=1
    fi
  done
done

((checked > 0)) || { echo "FAIL: no toolchain binaries found" >&2; exit 1; }
exit "$failed"
