#include <stddef.h>

#ifdef __ANDROID__
#include <android/log.h>
#include <android/native_activity.h>

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "DxApp", __VA_ARGS__)
#else
typedef struct ANativeActivity ANativeActivity;

#define LOG(...) ((void)0)
#endif

extern const char *app_greeting(void);

__attribute__((visibility("default"))) void
ANativeActivity_onCreate(ANativeActivity *activity, void *saved_state,
                         size_t saved_state_size) {
  (void)activity;
  (void)saved_state;
  (void)saved_state_size;
  LOG("greeting=%s", app_greeting());
}
