module Main (main) where

import Safe (headDef)

main :: IO ()
main = putStrLn (headDef "" ["stack"])
