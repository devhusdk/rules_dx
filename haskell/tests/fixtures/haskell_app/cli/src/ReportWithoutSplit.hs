-- | Imports Data.List.Split without depending on it; the build must fail.
module ReportWithoutSplit (fields) where

import Data.List.Split (splitOn)

fields :: String -> [String]
fields = splitOn ","
