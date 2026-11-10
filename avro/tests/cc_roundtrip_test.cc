#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <memory>

#include "avro/tests/shapes_cc.hh"
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

  dx_avro::Ring ring;
  ring.center = point;
  ring.radius = 9;

  std::unique_ptr<avro::OutputStream> out = avro::memoryOutputStream();
  avro::EncoderPtr encoder = avro::binaryEncoder();
  encoder->init(*out);
  avro::encode(*encoder, point);
  avro::encode(*encoder, segment);
  avro::encode(*encoder, ring);
  encoder->flush();

  std::unique_ptr<avro::InputStream> in = avro::memoryInputStream(*out);
  avro::DecoderPtr decoder = avro::binaryDecoder();
  decoder->init(*in);
  dx_avro::Point point2;
  dx_avro::Segment segment2;
  avro::decode(*decoder, point2);
  avro::decode(*decoder, segment2);
  dx_avro::Ring ring2;
  avro::decode(*decoder, ring2);

  if (point2.x != 3 || point2.y != -4 || segment2.length != 7 ||
      segment2.label != "edge" ||
      ring2.center.x != 3 || ring2.center.y != -4 || ring2.radius != 9) {
    std::cerr << "roundtrip mismatch\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
