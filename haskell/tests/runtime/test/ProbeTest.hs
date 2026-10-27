module Main (main) where

import Control.Monad (unless)
import Probe (add, bigPower, platform)
import System.Exit (exitFailure)

check :: String -> Bool -> IO ()
check name ok = unless ok (putStrLn ("FAIL: " ++ name) >> exitFailure)

main :: IO ()
main = do
  check "the C function adds" (add 40 2 == 42)
  check "Integer arithmetic runs on GMP" (show bigPower == "1267650600228229401496703205376")
  check "select() picked linux" (platform == "linux")
  contents <- readFile "haskell/tests/runtime/data.txt"
  check "the data runfile is readable" (lines contents == ["ledger"])
  putStrLn "PASS"
