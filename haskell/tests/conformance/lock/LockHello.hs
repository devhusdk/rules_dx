module Main (main) where

import Data.List.Split (splitOn)

main :: IO ()
main = putStrLn (unwords (splitOn "-" "hello-lock"))
