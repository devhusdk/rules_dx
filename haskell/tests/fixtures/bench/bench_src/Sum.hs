module Main (main) where

import Test.Tasty.Bench (bench, defaultMain, nf)

main :: IO ()
main = defaultMain [bench "sum" (nf (sum :: [Int] -> Int) [1 .. 1000])]
