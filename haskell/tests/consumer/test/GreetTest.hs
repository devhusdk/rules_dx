module Main (main) where

import Greet (greet)
import System.Exit (exitFailure)

main :: IO ()
main = if greet "world" == "hello world" then pure () else exitFailure
