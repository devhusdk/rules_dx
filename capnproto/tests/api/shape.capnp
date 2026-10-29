@0xd9e8f7a6b5c40833;

using Cxx = import "/capnp/c++.capnp";
$Cxx.namespace("dx::api");

using Common = import "/capnproto/tests/base/common.capnp";
using Kinds = import "kind.capnp";

struct Shape {
  name @0 :Text;
  origin @1 :Common.Point;
  kind @2 :Kinds.Kind;
}
