{-# LANGUAGE CPP #-}

module Probe (add, bigPower, platform) where

import Foreign.C.Types (CInt (..))

foreign import ccall unsafe "dx_probe_add" c_add :: CInt -> CInt -> CInt

-- | Adds two numbers in C through the FFI.
add :: Int -> Int -> Int
add a b = fromIntegral (c_add (fromIntegral a) (fromIntegral b))

-- | A power that needs GMP-backed Integer arithmetic.
bigPower :: Integer
bigPower = 2 ^ (100 :: Int)

-- | The operating system the select() picked.
platform :: String
#ifdef DX_LINUX
platform = "linux"
#else
platform = "other"
#endif
