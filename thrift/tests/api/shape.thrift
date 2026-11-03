namespace cpp dx.api
namespace java dx.api

include "thrift/tests/base/common.thrift"

struct Shape {
  1: string name,
  2: common.Point origin,
}

service Shapes {
  Shape get(1: string name),
}
