#include <cstdlib>
#include <exception>
#include <filesystem>
#include <iostream>
#include <memory>
#include <string>

#include "examples/cpp-app/app/report.h"
#include "examples/cpp-app/core/menu.h"
#include "rules_cc/cc/runfiles/runfiles.h"

namespace {

std::string from_invoking_directory(const std::string &path) {
  const char *base = std::getenv("BUILD_WORKING_DIRECTORY");
  if (base == nullptr || std::filesystem::path(path).is_absolute()) {
    return path;
  }
  return (std::filesystem::path(base) / path).string();
}

} // namespace

int main(int argc, char **argv) {
  using rules_cc::cc::runfiles::Runfiles;
  std::string path;
  if (argc > 1) {
    path = from_invoking_directory(argv[1]);
  } else {
    std::string error;
    std::unique_ptr<Runfiles> runfiles(
        Runfiles::Create(argv[0], BAZEL_CURRENT_REPOSITORY, &error));
    if (runfiles == nullptr) {
      std::cerr << "pantry: " << error << "\n";
      return 3;
    }
    path = runfiles->Rlocation(MENU_RLOCATION);
  }
  try {
    std::cout << pantry::render_report(pantry::load_menu(path));
  } catch (const std::exception &e) {
    std::cerr << "pantry: " << e.what() << "\n";
    return 4;
  }
  return 0;
}
