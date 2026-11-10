module Main (main) where

import Lib.Greeting (greeting)

main :: IO ()
main = putStrLn (greeting "wörld")
