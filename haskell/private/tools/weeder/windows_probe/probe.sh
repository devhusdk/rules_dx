#!/usr/bin/env bash
set -u
runfile() { local p; p=$(find -L . -path "*$1" -print -quit 2>/dev/null); [ -n "$p" ] && echo "$PWD/${p#./}"; }
cd "${RUNFILES_DIR:-$0.runfiles}" 2>/dev/null || true
weeder=$(runfile "/_install/bin/weeder.exe")
ghc=$(runfile "dx_ghc_windows_x86_64/bin/ghc.exe")
bin=$(dirname "$(runfile "/mingw/bin/clang.exe")")
echo "weeder=$weeder ghc=$ghc mingw_bin=$bin"
work="${TEST_TMPDIR:-$(mktemp -d)}"
mkdir -p "$work/src" "$work/hie"
printf 'module Main (main) where\n\nunused :: Int\nunused = 1\n\nmain :: IO ()\nmain = pure ()\n' > "$work/src/Main.hs"
printf 'roots = ["^Main.main$"]\ntype-class-roots = true\n' > "$work/weeder.toml"
echo "== ghc -fno-code -fwrite-ide-info"
( cd "$work" && "$ghc" -fno-code -fwrite-ide-info -hiedir hie src/Main.hs; echo "exit $?" )
for args in "--help" "--version" "--version +RTS -N1 -RTS" "--hie-directory $work/hie" "--hie-directory $work/hie +RTS -N1 -RTS"; do
  echo "== weeder $args"
  ( cd "$work" && "$weeder" $args; echo "exit $?" ) 2>&1 | tee -a "$work/crash.txt" | head -40
done
od=$bin/llvm-objdump.exe
ro=$bin/llvm-readobj.exe
pref=$("$od" -p "$weeder" | awk '/ImageBase/ {print $2; exit}')
pref=$((0x$pref))
echo "== sections"
"$ro" --sections "$weeder" | grep -E "^ +(Name|VirtualSize|VirtualAddress|RawDataSize|PointerToRawData):"
line=$(grep -m1 -o '0x[0-9a-f]* *[^ ]*weeder.exe+0x[0-9a-f]*' "$work/crash.txt")
abs=$(echo "$line" | awk '{print $1}')
rel=$(echo "$line" | sed 's/.*+//')
crash=$(grep -m1 -o 'executing data at 0x[0-9a-f]*' "$work/crash.txt" | awk '{print $4}')
[ -n "$crash" ] && [ -n "$abs" ] || { echo "no crash frame"; exit 1; }
base=$((abs - rel))
off=$((crash - base))
low=$((off & 0xffffffff))
printf '== load base 0x%x crash offset 0x%x low32 0x%x pref 0x%x\n' $base $off $low $pref
bad=$((pref + off)); good=$((pref + low))
printf '== disasm at intended target 0x%x\n' $good
"$od" -d --start-address=$((good - 0x40)) --stop-address=$((good + 0x40)) "$weeder" | tail -40
echo "== qwords in file equal to bad 0x$(printf %x $bad) or good 0x$(printf %x $good)"
od -A x -t x8 -v "$weeder" | awk -v a="$(printf %016x $bad)" -v b="$(printf %016x $good)" '{for(i=2;i<=NF;i++) if($i==a||$i==b) {printf "fileoff 0x%x %s\n", strtonum("0x"$1)+(i-2)*8, $i; n++} if(n>20) exit}'
echo "== dwords in file equal to 0x$(printf %x $low) or rva-from-low32 0x$(printf %x $((low)))"
od -A x -t x4 -v "$weeder" | awk -v a="$(printf %08x $low)" -v b="$(printf %08x $(((pref + low) & 0xffffffff)))" '{for(i=2;i<=NF;i++) if($i==a||$i==b) {printf "fileoff 0x%x %s\n", strtonum("0x"$1)+(i-2)*4, $i; n++} if(n>20) exit}'
echo "== text references to 0x$(printf %x $good)"
timeout 600 "$od" -d --no-show-raw-insn "$weeder" | grep -i -B12 "$(printf %x $good)" | head -120
echo "== static closure bytes at rva 0x$(printf %x $((rel & ~7)))"
"$od" -s --start-address=$((pref + (rel & ~7) - 0x20)) --stop-address=$((pref + (rel & ~7) + 0x40)) "$weeder" | tail -8
echo "== base relocs, count by type"
"$ro" --coff-basereloc "$weeder" | grep -E "Type:" | sort | uniq -c
exit 1
