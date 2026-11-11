module Main (main) where

import Greet (greeting)
import System.Exit (exitFailure)

main :: IO ()
main
  | greeting == "hello world" = putStrLn "PASS passing_case"
  | otherwise = putStrLn "FAIL passing_case" >> exitFailure
