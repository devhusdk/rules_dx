@0xdbb9ad1f14bf0b38;

using Point = import "point.capnp";

struct Path {
  points @0 :List(Point.Point);
}
