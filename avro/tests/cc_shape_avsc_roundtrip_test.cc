#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <memory>

#include "avro/tests/shape_avsc_cc.hh"
#include <avro/Decoder.hh>
#include <avro/Encoder.hh>
#include <avro/Stream.hh>

int main() {
  dx_test::Shape shape;
  shape.name = "circle";
  shape.base.id = 7;

  std::unique_ptr<avro::OutputStream> out = avro::memoryOutputStream();
  avro::EncoderPtr encoder = avro::binaryEncoder();
  encoder->init(*out);
  avro::encode(*encoder, shape);
  encoder->flush();

  std::unique_ptr<avro::InputStream> in = avro::memoryInputStream(*out);
  avro::DecoderPtr decoder = avro::binaryDecoder();
  decoder->init(*in);
  dx_test::Shape shape2;
  avro::decode(*decoder, shape2);

  if (shape2.name != "circle" || shape2.base.id != 7) {
    std::cerr << "roundtrip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
