module Main (main) where

import Greet (greeting)
import System.Exit (exitFailure)

main :: IO ()
main
  | greeting == "goodbye" = putStrLn "PASS failing_case"
  | otherwise = putStrLn "FAIL failing_case" >> exitFailure
