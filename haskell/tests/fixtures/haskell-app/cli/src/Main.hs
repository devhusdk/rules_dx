module Main (main) where

import System.Exit (ExitCode (..), exitWith)
import System.IO (hPutStr, stderr)

import Cli (run)

main :: IO ()
main = do
  input <- getContents
  let (code, out, err) = run input
  putStr out
  hPutStr stderr err
  exitWith (if code == 0 then ExitSuccess else ExitFailure code)
