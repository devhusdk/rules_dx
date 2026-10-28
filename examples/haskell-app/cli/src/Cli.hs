{-# LANGUAGE CPP #-}

-- | The stockctl logic: a pure function from input text to exit code and output.
module Cli (stockLimit, run) where

import Stock (Item (..), parseItems, render)

#ifndef STOCK_LIMIT
#define STOCK_LIMIT 60
#endif

-- | Largest quantity accepted for one item; set with @--//examples/haskell-app/cli:limit@.
stockLimit :: Int
stockLimit = STOCK_LIMIT

-- | Returns the exit code, standard output and standard error.
run :: String -> (Int, String, String)
run input = case parseItems input of
  Left err -> (2, "", "stockctl: " ++ err ++ "\n")
  Right items -> case [itemName i | i <- items, itemQty i > stockLimit] of
    [] -> (0, render items, "")
    (name : _) -> (4, "", "stockctl: " ++ name ++ " exceeds limit " ++ show stockLimit ++ "\n")
