@0xd0b9ad1f14bf0b37;

using Cxx = import "/capnp/c++.capnp";
$Cxx.namespace("demo");
using Base = import "base.capnp";

struct Top {
  base @0 :Base.Base;
}
