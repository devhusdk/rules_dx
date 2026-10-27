module Main (main) where

import Control.Monad (unless)
import System.Exit (exitFailure)

import Stock

check :: String -> Bool -> IO Bool
check name ok = do
  unless ok (putStrLn ("FAIL: " ++ name))
  pure ok

main :: IO ()
main = do
  let input = "lamp,4,1250\ncable,12,199\n\nfuse,0,35\n"
  results <-
    sequence
      [ check "parse" (parseItems input == Right [Item "lamp" 4 1250, Item "cable" 12 199, Item "fuse" 0 35])
      , check "total" (fmap grandTotal (parseItems input) == Right 7388)
      , check "malformed" (parseItems "lamp,4\n" == Left "malformed line 1")
      , check "not a number" (parseItems "a,1,2\nb,x,2\n" == Left "malformed line 2")
      , check "version" (version == "1.0.0")
      , check "checksum" (checksumOf "Wikipedia" == 0x11E60398)
      ]
  unless (and results) exitFailure
