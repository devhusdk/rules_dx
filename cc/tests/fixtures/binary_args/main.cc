#include <cstring>
#include <fstream>

int main(int argc, char **argv) {
  if (argc != 3 || std::strcmp(argv[1], "--marker") != 0)
    return 1;
  std::ifstream in(argv[2]);
  return in.good() ? 0 : 2;
}
