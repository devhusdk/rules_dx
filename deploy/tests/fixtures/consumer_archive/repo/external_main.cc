#include <fstream>
#include <iostream>
#include <string>

#include "external_greet.h"

namespace {

std::string DirName(const std::string& path) {
  size_t pos = path.find_last_of("/\\");
  if (pos == std::string::npos) {
    return ".";
  }
  if (pos == 0) {
    return path.substr(0, 1);
  }
  return path.substr(0, pos);
}

}  // namespace

int main(int argc, char** argv) {
  std::string exe = (argc > 0 && argv[0] != nullptr) ? argv[0] : ".";
  std::string banner = DirName(exe) + "/resources/banner.txt";
  std::ifstream in(banner);
  if (!in) {
    std::cerr << "external_app: cannot read " << banner << "\n";
    return 1;
  }
  external_greet_state* state = external_greet_create("external");
  if (state == nullptr) {
    std::cerr << "external_app: native create failed\n";
    return 1;
  }
  std::string line;
  while (std::getline(in, line)) {
    std::cout << "external_app resource: " << line << "\n";
  }
  std::cout << "external_app native: " << external_greet_message(state) << "\n";
  external_greet_destroy(state);
  return 0;
}
