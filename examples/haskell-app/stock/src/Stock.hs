-- | Parses stock lists of the form @name,quantity,price@ per line.
module Stock
  ( Item (..)
  , parseItems
  , lineTotal
  , grandTotal
  , render
  , checksumOf
  , version
  ) where

import Data.List.Split (splitOn)
import Data.Word (Word32)
import Foreign.C.String (CStringLen, withCStringLen)
import Foreign.C.Types (CChar, CSize (..))
import Foreign.Ptr (Ptr)
import System.IO.Unsafe (unsafePerformIO)
import Text.Read (readMaybe)

import qualified Stock.Version as V

data Item = Item
  { itemName :: String
  , itemQty :: Int
  , itemPrice :: Int
  }
  deriving (Eq, Show)

foreign import ccall unsafe "stock_checksum"
  c_checksum :: Ptr CChar -> CSize -> IO Word32

-- | Parses all non-blank lines; the error names the first malformed line.
parseItems :: String -> Either String [Item]
parseItems = mapM parseLine . filter (not . null . snd) . zip [1 :: Int ..] . lines
  where
    parseLine (n, l) = case splitOn "," l of
      [name, q, p]
        | not (null name), Just qty <- readMaybe q, Just price <- readMaybe p ->
            Right (Item name qty price)
      _ -> Left ("malformed line " ++ show n)

lineTotal :: Item -> Int
lineTotal i = itemQty i * itemPrice i

grandTotal :: [Item] -> Int
grandTotal = sum . map lineTotal

render :: [Item] -> String
render items =
  unlines
    ([name ++ " " ++ show q ++ " x " ++ show p ++ " = " ++ show (q * p) | Item name q p <- items]
       ++ ["total " ++ show (grandTotal items)])

-- | Adler-32 checksum computed by the C library.
checksumOf :: String -> Word32
checksumOf s = unsafePerformIO (withCStringLen s call)
  where
    call :: CStringLen -> IO Word32
    call (ptr, len) = c_checksum ptr (fromIntegral len)

version :: String
version = V.version
