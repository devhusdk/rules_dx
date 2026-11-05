#include <cstdio>
#include <string>

#include "summary.h"

static int expect(const char *what, const std::string &got,
                  const std::string &want) {
  if (got != want) {
    std::fprintf(stderr, "%s: got '%s', want '%s'\n", what, got.c_str(),
                 want.c_str());
    return 1;
  }
  return 0;
}

int main() {
  int failures = 0;
  failures += expect("hello", adopt::Summarize("hello world"),
                     "words=2 adler32=1a0b045d");
  failures += expect("empty", adopt::Summarize(""), "words=0 adler32=00000001");
  return failures;
}
