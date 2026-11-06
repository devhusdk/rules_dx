module Hello (hello) where

import qualified Data.ByteString.Char8 as B
import qualified Data.Map.Strict as Map
import Data.Text (Text)

hello :: Map.Map Text B.ByteString
hello = Map.empty
