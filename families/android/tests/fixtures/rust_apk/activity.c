#include <stddef.h>
#include <stdint.h>

#ifdef __ANDROID__
#include <android/asset_manager.h>
#include <android/input.h>
#include <android/log.h>
#include <android/looper.h>
#include <android/native_activity.h>

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "DxRustApk", __VA_ARGS__)
#else
typedef struct ANativeActivity ANativeActivity;

struct ANativeActivity {
  void *instance;
};

#define LOG(...) ((void)0)
#endif

extern int app_main(void);
extern int app_lifecycle(int event);
extern int app_touch(void);
extern int app_key(void);
extern int app_asset_check(const uint8_t *data, size_t len);

enum {
  kCreated = 0,
  kStarted = 1,
  kResumed = 2,
  kPaused = 3,
  kStopped = 4,
  kDestroyed = 5,
};

#ifdef __ANDROID__
static void report(const char *name, int event) {
  LOG("lifecycle=%s state=%d", name, app_lifecycle(event));
}

static void on_start(ANativeActivity *activity) {
  (void)activity;
  report("start", kStarted);
}

static void on_resume(ANativeActivity *activity) {
  (void)activity;
  report("resume", kResumed);
}

static void on_pause(ANativeActivity *activity) {
  (void)activity;
  report("pause", kPaused);
}

static void on_stop(ANativeActivity *activity) {
  (void)activity;
  report("stop", kStopped);
}

static void on_destroy(ANativeActivity *activity) {
  (void)activity;
  report("destroy", kDestroyed);
}

static int drain_input(int fd, int events, void *data) {
  (void)fd;
  (void)events;
  AInputQueue *queue = (AInputQueue *)data;
  AInputEvent *event = NULL;
  while (AInputQueue_getEvent(queue, &event) >= 0) {
    if (AInputQueue_preDispatchEvent(queue, event)) {
      continue;
    }
    int type = AInputEvent_getType(event);
    if (type == AINPUT_EVENT_TYPE_MOTION) {
      LOG("input=touch total=%d", app_touch());
    } else if (type == AINPUT_EVENT_TYPE_KEY) {
      LOG("input=key total=%d", app_key());
    }
    AInputQueue_finishEvent(queue, event, 1);
  }
  return 1;
}

static void on_input_queue_created(ANativeActivity *activity,
                                   AInputQueue *queue) {
  (void)activity;
  AInputQueue_attachLooper(queue, ALooper_forThread(), ALOOPER_POLL_CALLBACK,
                           drain_input, queue);
}

static void on_input_queue_destroyed(ANativeActivity *activity,
                                     AInputQueue *queue) {
  (void)activity;
  AInputQueue_detachLooper(queue);
}

static void load_asset(ANativeActivity *activity) {
  AAsset *asset = AAssetManager_open(activity->assetManager, "message.txt",
                                     AASSET_MODE_BUFFER);
  if (asset == NULL) {
    LOG("asset=missing name=message.txt");
    return;
  }
  const uint8_t *bytes = (const uint8_t *)AAsset_getBuffer(asset);
  size_t length = (size_t)AAsset_getLength(asset);
  int mismatch = bytes == NULL ? 1 : app_asset_check(bytes, length);
  LOG("asset=%s name=message.txt bytes=%zu", mismatch ? "mismatch" : "ok",
      length);
  AAsset_close(asset);
}
#endif

__attribute__((visibility("default"))) void
ANativeActivity_onCreate(ANativeActivity *activity, void *saved_state,
                         size_t saved_state_size) {
  (void)saved_state;
  (void)saved_state_size;
  int status = app_main();
  activity->instance = (void *)(intptr_t)status;
  LOG("lifecycle=create state=%d status=%d", kCreated, status);
#ifdef __ANDROID__
  activity->callbacks->onStart = on_start;
  activity->callbacks->onResume = on_resume;
  activity->callbacks->onPause = on_pause;
  activity->callbacks->onStop = on_stop;
  activity->callbacks->onDestroy = on_destroy;
  activity->callbacks->onInputQueueCreated = on_input_queue_created;
  activity->callbacks->onInputQueueDestroyed = on_input_queue_destroyed;
  load_asset(activity);
#endif
}
