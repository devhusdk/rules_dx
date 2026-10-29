@0xb7c4a3d2e1f09a11;

using Cxx = import "/capnp/c++.capnp";
using Java = import "/capnp/java.capnp";
$Cxx.namespace("dx::base");
$Java.package("dx.base");
$Java.outerClassname("CommonSchema");

struct Point {
  x @0 :Float32;
  y @1 :Float32;
}
