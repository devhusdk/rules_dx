#!/usr/bin/env bash
set -u
runfile() { local p; p=$(find -L . -path "*$1" -print -quit 2>/dev/null); [ -n "$p" ] && echo "$PWD/${p#./}"; }
cd "${RUNFILES_DIR:-$0.runfiles}" 2>/dev/null || true
weeder=$(runfile "/_install/bin/weeder.exe")
ghc=$(runfile "dx_ghc_windows_x86_64/bin/ghc.exe")
bin=$(dirname "$(runfile "/mingw/bin/clang.exe")")
export PATH="$bin:$PATH"
od=$bin/llvm-objdump.exe
ro=$bin/llvm-readobj.exe
echo "weeder=$weeder ghc=$ghc mingw_bin=$bin"
work="${TEST_TMPDIR:-$(mktemp -d)}"
echo "== ghc --info"
"$ghc" --info | grep -E -i "assembler|linker|ld |C compiler|PIC|split|merge objects|target"
pref=$((0x$("$od" -p "$weeder" | awk '/ImageBase/ {print $2; exit}')))
echo "== weeder jump table used at text rva 0x1cc292 caller"
"$od" -d --start-address=$((pref + 0x1cc2a0)) --stop-address=$((pref + 0x1cc2b4)) "$weeder" | grep -o '# 0x[0-9a-f]*' | head -1 | while read -r _ t; do
  "$od" -s --start-address=$((t)) --stop-address=$((t + 0x60)) "$weeder" | tail -7
done
cat > "$work/Jt.hs" <<'HS'
module Main (main) where
import System.Environment (getArgs)
data T = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 deriving (Enum, Show)
{-# NOINLINE f #-}
f :: T -> Int
f t = case t of
  A0 -> 10; A1 -> 21; A2 -> 32; A3 -> 43; A4 -> 54
  A5 -> 65; A6 -> 76; A7 -> 87; A8 -> 98; A9 -> 109
main :: IO ()
main = do
  as <- getArgs
  mapM_ (print . f . toEnum . length) (["", "a", "aaaaaaaaa"] ++ as)
HS
i=0
for v in "" "-fno-PIC" "-fPIC" "-fsplit-sections" "-fno-split-sections" "-fexternal-dynamic-refs" "-fPIC -fsplit-sections -fexternal-dynamic-refs"; do
  i=$((i + 1))
  echo "== variant $i: ghc -O $v"
  mkdir -p "$work/o$i"
  ( cd "$work" && "$ghc" -O $v -outputdir "o$i" -o "jt$i.exe" Jt.hs >/dev/null 2>&1; echo "build exit $?" )
  ( cd "$work" && ./"jt$i.exe" 2>&1 | head -8; echo "run exit $?" )
  ( cd "$work" && "$ghc" -O $v -S -outputdir "o$i" -o "o$i/Main.s" Jt.hs >/dev/null 2>&1 )
  s=$(ls "$work"/o$i/*.s 2>/dev/null | head -1)
  [ -n "$s" ] && grep -n -E "\.section|\.quad\s+\S+-\S+|\.long\s+\S+-\S+" "$s" | grep -B1 -E "\.quad|\.long" | head -14
  [ -f "$work/o$i/Main.o" ] && "$ro" -r "$work/o$i/Main.o" | grep -E "Section|IMAGE_REL" | awk '{print $1, $2, $3}' | sort | uniq -c | head -20
done
echo "== weeder crash for reference"
( cd "$work" && "$weeder" --version 2>&1 | head -3 )
exit 1
