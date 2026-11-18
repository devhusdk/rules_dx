@0xc1d2e3f4a5b60722;

using Cxx = import "/capnp/c++.capnp";
using Java = import "/capnp/java.capnp";
$Cxx.namespace("dx::api");
$Java.package("dx.api");
$Java.outerClassname("KindSchema");

enum Kind {
  circle @0;
  square @1;
}
