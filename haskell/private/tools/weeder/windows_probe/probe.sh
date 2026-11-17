#!/usr/bin/env bash
set -u
runfile() { local p; p=$(find -L . -path "*$1" -print -quit 2>/dev/null); [ -n "$p" ] && echo "$PWD/${p#./}"; }
cd "${RUNFILES_DIR:-$0.runfiles}" 2>/dev/null || true
weeder=$(runfile "/_install/bin/weeder.exe")
ghc=$(runfile "dx_ghc_windows_x86_64/bin/ghc.exe")
bin=$(dirname "$(runfile "/mingw/bin/clang.exe")")
echo "weeder=$weeder ghc=$ghc mingw_bin=$bin"
ls "$bin" | tr '\n' ' '; echo
work="${TEST_TMPDIR:-$(mktemp -d)}"
mkdir -p "$work/src" "$work/hie"
printf 'module Main (main) where\n\nunused :: Int\nunused = 1\n\nmain :: IO ()\nmain = pure ()\n' > "$work/src/Main.hs"
echo "== ghc -fno-code -fwrite-ide-info"
( cd "$work" && "$ghc" -fno-code -fwrite-ide-info -hiedir hie src/Main.hs; echo "exit $?" )
find "$work/hie" -type f
for args in "--version" "--hie-directory $work/hie" "--hie-directory $work/hie +RTS -N1 -RTS" "--hie-directory $work/hie +RTS -N1 -V0 -RTS"; do
  echo "== weeder $args"
  ( cd "$work" && "$weeder" $args; echo "exit $?" ) 2>&1 | tee -a "$work/crash.txt"
done
for tool in llvm-objdump objdump; do
  [ -x "$bin/$tool.exe" ] || continue
  echo "== $tool -p"
  "$bin/$tool.exe" -p "$weeder" | grep -i -E "ImageBase|DllCharacteristics|SectionAlignment|^Sections|^ *[0-9]+ \." | head -20
  "$bin/$tool.exe" -h "$weeder" | head -30
  base=$("$bin/$tool.exe" -p "$weeder" | awk '/ImageBase/ {print $2; exit}')
  for rva in $(grep -o 'weeder.exe+0x[0-9a-f]*' "$work/crash.txt" | sed 's/.*+//' | sort -u | head -6); do
    addr=$(printf '0x%x' $((0x$base + rva)))
    echo "== $tool -d around rva $rva addr $addr"
    "$bin/$tool.exe" -d --no-show-raw-insn --start-address=$((addr - 0x60)) --stop-address=$((addr + 0x20)) "$weeder" | tail -40
  done
  for target in $(grep -o 'executing data at 0x[0-9a-f]*' "$work/crash.txt" | awk '{print $4}' | sort -u | head -2); do
    echo "== crash target $target, symbols with the same low 16 bits"
    "$bin/$tool.exe" -t "$weeder" | grep -i "$(printf '%04x' $((target & 0xffff)))" | head -10
  done
  break
done
for tool in llvm-nm nm; do
  [ -x "$bin/$tool.exe" ] || continue
  echo "== $tool getOrSet / __imp_ symbols"
  "$bin/$tool.exe" "$weeder" | grep -E "getOrSetLibHS|__imp_getOrSet|__imp_.*(rts|Rts|stg|Stg)" | head -40
  break
done
exit 1
