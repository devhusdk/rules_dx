@0xc1d2e3f4a5b60722;

using Cxx = import "/capnp/c++.capnp";
$Cxx.namespace("dx::api");

enum Kind {
  circle @0;
  square @1;
}
