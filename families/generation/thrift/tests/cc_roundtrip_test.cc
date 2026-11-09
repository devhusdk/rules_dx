#include <cstdlib>
#include <iostream>
#include <memory>

#include <thrift/protocol/TBinaryProtocol.h>
#include <thrift/transport/TBufferTransports.h>

#include "common_types.h"
#include "shape_types.h"

int main() {
  using apache::thrift::protocol::TBinaryProtocol;
  using apache::thrift::transport::TMemoryBuffer;

  dx::api::Shape shape;
  shape.name = "circle";
  shape.origin.x = 1.5;
  shape.origin.y = -2.0;

  auto buffer = std::make_shared<TMemoryBuffer>();
  TBinaryProtocol out(buffer);
  shape.write(&out);

  dx::api::Shape read;
  TBinaryProtocol in(buffer);
  read.read(&in);
  if (read.name != "circle" || read.origin.x != 1.5 || read.origin.y != -2.0) {
    std::cerr << "round trip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
