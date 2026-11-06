#include <iostream>
#include <string>

#include "api/v1/greeting.pb.h"

int main() {
  api::v1::Greeting greeting;
  greeting.set_name("cc");
  greeting.set_times(2);
  std::string data;
  greeting.SerializeToString(&data);
  api::v1::Greeting back;
  if (!back.ParseFromString(data)) {
    return 1;
  }
  std::cout << back.name() << " " << back.times() << "\n";
  return 0;
}
