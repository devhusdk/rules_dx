#include <capnp/message.h>
#include <capnp/serialize.h>

#include <cstdlib>
#include <iostream>

#include "capnproto/tests/api/kind.capnp.h"
#include "capnproto/tests/api/shape.capnp.h"
#include "capnproto/tests/base/common.capnp.h"

int main() {
  capnp::MallocMessageBuilder builder;
  auto shape = builder.initRoot<dx::api::Shape>();
  shape.setName("circle");
  shape.getOrigin().setX(1.5F);
  shape.getOrigin().setY(-2.0F);
  shape.setKind(dx::api::Kind::SQUARE);

  auto words = capnp::messageToFlatArray(builder);
  capnp::FlatArrayMessageReader reader(words);
  auto read = reader.getRoot<dx::api::Shape>();
  if (read.getName() != "circle" || read.getOrigin().getX() != 1.5F ||
      read.getOrigin().getY() != -2.0F ||
      read.getKind() != dx::api::Kind::SQUARE) {
    std::cerr << "round trip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
