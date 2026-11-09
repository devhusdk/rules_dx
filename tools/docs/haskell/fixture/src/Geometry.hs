-- | Geometry helpers.
module Geometry
  ( Shape (..)
  , Circle (..)
  , Area (..)
  , area
  , unitCircle
  ) where

-- | A circle.
data Circle = Circle
  { radius :: Double
  -- ^ The radius.
  }

-- | A shape with an area.
data Shape = Square Double | Round Circle

-- | Things that have an area.
class Area a where
  -- | The area.
  areaOf :: a -> Double

-- | Area of a circle.
area :: Circle -> Double
area (Circle r) = pi * r * r

-- | The circle of radius one.
unitCircle :: Circle
unitCircle = Circle 1

instance Area Circle where
  areaOf = area
