module Main (main) where

import Greeter (greet)
import System.Exit (exitFailure)

main :: IO ()
main = if greet "dx" == "hello dx" then pure () else exitFailure
