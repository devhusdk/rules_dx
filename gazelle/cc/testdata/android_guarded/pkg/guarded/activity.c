#include <stddef.h>

#ifdef __ANDROID__
#include <android/log.h>
#include <android/looper.h>
#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "guarded", __VA_ARGS__)
#else
#define LOG(...) ((void)0)
#endif

int GuardedTick(void) {
  LOG("tick");
  return 1;
}
