#include <cstdio>
#include <cstring>
#include <string>
#include <thread>

#if defined(__has_feature)
#if __has_feature(thread_sanitizer)
#define DX_TSAN 1
#endif
#endif
#ifdef __SANITIZE_THREAD__
#define DX_TSAN 1
#endif
#ifndef DX_TSAN
#error "expected ThreadSanitizer to be enabled"
#endif

namespace {
int shared_counter = 0;

int race() {
  std::thread t1([] { for (int i = 0; i < 1000; ++i) ++shared_counter; });
  std::thread t2([] { for (int i = 0; i < 1000; ++i) ++shared_counter; });
  t1.join();
  t2.join();
  return 0;
}
}  // namespace

int main(int argc, char** argv) {
  if (argc > 1) return race();

  std::string command = std::string("\"") + argv[0] + "\" race 2>&1";
  FILE* pipe = popen(command.c_str(), "r");
  if (pipe == nullptr) return 2;
  std::string output;
  char buffer[512];
  while (fgets(buffer, sizeof buffer, pipe) != nullptr) output += buffer;
  int status = pclose(pipe);
  bool reported = output.find("ThreadSanitizer: data race") != std::string::npos;
  return (reported && status != 0) ? 0 : 1;
}
