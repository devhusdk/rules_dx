module Main (main) where

import Probe (add, bigPower, platform)

main :: IO ()
main = do
  putStrLn (platform ++ " " ++ show (add 40 2))
  print bigPower
