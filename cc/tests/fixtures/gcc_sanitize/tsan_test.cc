#include <atomic>
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

int main() {
  std::atomic<int> counter{0};
  std::thread t1([&] { for (int i = 0; i < 1000; ++i) ++counter; });
  std::thread t2([&] { for (int i = 0; i < 1000; ++i) ++counter; });
  t1.join();
  t2.join();
  return counter.load() == 2000 ? 0 : 1;
}
