module Main (main) where

import Data.List (isPrefixOf)
import Greet (greeting)
import System.Environment (getArgs, lookupEnv)

main :: IO ()
main = do
  args <- getArgs
  case args of
    [path] -> rlocation path >>= readFile >>= putStr
    _ -> putStrLn greeting

rlocation :: String -> IO FilePath
rlocation path = do
  dir <- lookupEnv "RUNFILES_DIR"
  case dir of
    Just root | not (null root) -> pure (root <> "/" <> path)
    _ -> lookupEnv "RUNFILES_MANIFEST_FILE" >>= maybe (fail "no runfiles") (manifestLookup path)

manifestLookup :: String -> FilePath -> IO FilePath
manifestLookup path manifest = do
  text <- readFile manifest
  let key = path <> " "
  case [filter (/= '\r') (drop (length key) line) | line <- lines text, key `isPrefixOf` line] of
    (target : _) -> pure target
    [] -> fail ("not in runfiles manifest: " <> path)
