module Main (main) where

import Control.Monad (unless)
import System.Environment (getArgs)
import System.Exit (exitFailure)

import Cli (run, stockLimit)

check :: String -> Bool -> IO Bool
check name ok = do
  unless ok (putStrLn ("FAIL: " ++ name))
  pure ok

main :: IO ()
main = do
  want <- (read . concat . take 1) <$> getArgs
  let (okCode, okOut, _) = run "lamp,4,1250\ncable,12,199\n"
      (badCode, _, badErr) = run "lamp,4\n"
      (bigCode, _, _) = run "crate,100,1\n"
  results <-
    sequence
      [ check "limit" (stockLimit == want)
      , check "valid exit" (okCode == 0)
      , check "valid output" (okOut == "lamp 4 x 1250 = 5000\ncable 12 x 199 = 2388\ntotal 7388\n")
      , check "malformed exit" (badCode == 2)
      , check "malformed message" (badErr == "stockctl: malformed line 1\n")
      , check "limit exit" (bigCode == (if 100 > want then 4 else 0))
      ]
  unless (and results) exitFailure
