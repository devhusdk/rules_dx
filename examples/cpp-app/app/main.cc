#include <exception>
#include <iostream>
#include <string>

#include "examples/cpp-app/app/report.h"
#include "examples/cpp-app/core/menu.h"
#include "rules_cc/cc/runfiles/runfiles.h"

int main(int argc, char** argv) {
  using rules_cc::cc::runfiles::Runfiles;
  std::string error;
  std::unique_ptr<Runfiles> runfiles(Runfiles::Create(argv[0], &error));
  if (runfiles == nullptr) {
    std::cerr << "pantry: " << error << "\n";
    return 3;
  }
  const std::string path = argc > 1 ? argv[1]
                                    : runfiles->Rlocation(
                                          "_main/examples/cpp-app/core/menu.csv");
  try {
    std::cout << pantry::render_report(pantry::load_menu(path));
  } catch (const std::exception& e) {
    std::cerr << "pantry: " << e.what() << "\n";
    return 4;
  }
  return 0;
}
