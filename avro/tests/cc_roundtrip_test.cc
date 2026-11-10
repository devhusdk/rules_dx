#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <memory>

#include "avro/tests/point.hh"
#include "avro/tests/shape.hh"
#include <avro/Decoder.hh>
#include <avro/Encoder.hh>
#include <avro/Stream.hh>

int main() {
  dx_avro::Point point;
  point.x = 3;
  point.y = -4;
  dx_avro::Segment segment;
  segment.length = 7;
  segment.label = "edge";

  std::unique_ptr<avro::OutputStream> out = avro::memoryOutputStream();
  avro::EncoderPtr encoder = avro::binaryEncoder();
  encoder->init(*out);
  avro::encode(*encoder, point);
  avro::encode(*encoder, segment);
  encoder->flush();

  std::unique_ptr<avro::InputStream> in = avro::memoryInputStream(*out);
  avro::DecoderPtr decoder = avro::binaryDecoder();
  decoder->init(*in);
  dx_avro::Point point2;
  dx_avro::Segment segment2;
  avro::decode(*decoder, point2);
  avro::decode(*decoder, segment2);

  if (point2.x != 3 || point2.y != -4 || segment2.length != 7 ||
      segment2.label != "edge") {
    std::cerr << "roundtrip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
