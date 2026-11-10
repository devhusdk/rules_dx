#include <cstdlib>
#include <iostream>

#include "flatbuffers/flatbuffers.h"
#include "flatbuffers/tests/api/shape_generated.h"
#include "flatbuffers/tests/base/common_generated.h"

int main() {
  flatbuffers::FlatBufferBuilder builder;
  dx::base::Point origin(1.5F, -2.0F);
  auto name = builder.CreateString("circle");
  builder.Finish(dx::api::CreateShape(builder, name, &origin));

  flatbuffers::Verifier verifier(builder.GetBufferPointer(), builder.GetSize());
  if (!dx::api::VerifyShapeBuffer(verifier)) {
    std::cerr << "verifier rejected the buffer\n";
    return EXIT_FAILURE;
  }
  const dx::api::Shape *shape = dx::api::GetShape(builder.GetBufferPointer());
  if (shape->name()->str() != "circle" || shape->origin()->x() != 1.5F ||
      shape->origin()->y() != -2.0F) {
    std::cerr << "round trip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
