#include <stddef.h>

#include <android/log.h>
#include <android/native_activity.h>

void ANativeActivity_onCreate(ANativeActivity* activity, void* state, size_t size) {
  __android_log_print(ANDROID_LOG_INFO, "native", "created");
}
