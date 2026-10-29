@0xd9e8f7a6b5c40833;

using Cxx = import "/capnp/c++.capnp";
using Java = import "/capnp/java.capnp";
$Cxx.namespace("dx::api");
$Java.package("dx.api");
$Java.outerClassname("ShapeSchema");

using Common = import "/capnproto/tests/base/common.capnp";
using Kinds = import "kind.capnp";

struct Shape {
  name @0 :Text;
  origin @1 :Common.Point;
  kind @2 :Kinds.Kind;
}
