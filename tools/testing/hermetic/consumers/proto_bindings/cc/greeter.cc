#include "cc/greeter.h"

bool RoundTrip(const std::string& name, int times, api::v1::Greeting* out) {
  api::v1::Greeting greeting;
  greeting.set_name(name);
  greeting.set_times(times);
  std::string data;
  return greeting.SerializeToString(&data) && out->ParseFromString(data);
}
