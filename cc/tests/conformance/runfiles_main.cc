#include <fstream>
#include <iostream>
#include <memory>
#include <sstream>
#include <string>

#include "rules_cc/cc/runfiles/runfiles.h"

int main(int argc, char **argv) {
  using rules_cc::cc::runfiles::Runfiles;
  std::string error;
  std::unique_ptr<Runfiles> runfiles(
      Runfiles::Create(argv[0], BAZEL_CURRENT_REPOSITORY, &error));
  if (runfiles == nullptr || argc < 2) {
    std::cerr << error << std::endl;
    return 1;
  }
  std::ifstream data(runfiles->Rlocation(argv[1]));
  if (!data) {
    std::cerr << "cannot open " << argv[1] << std::endl;
    return 1;
  }
  std::stringstream content;
  content << data.rdbuf();
  std::cout << content.str();
  return 0;
}
