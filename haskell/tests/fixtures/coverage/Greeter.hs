module Greeter (greet, unused) where

greet :: String -> String
greet name = "hello " ++ name

unused :: String
unused = "never called"
