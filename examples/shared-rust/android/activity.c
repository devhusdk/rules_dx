#include <android/asset_manager.h>
#include <android/input.h>
#include <android/log.h>
#include <android/looper.h>
#include <android/native_activity.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "tasks.h"

#define LOG(...)                                                               \
  __android_log_print(ANDROID_LOG_INFO, "DxSharedTasks", __VA_ARGS__)

static void log_summary(void) {
  char summary[128];
  tasks_summary((uint8_t *)summary, sizeof(summary));
  LOG("summary %s", summary);
}

static void run(const char *command, const char *source) {
  char reply[128];
  tasks_apply((const uint8_t *)command, strlen(command), (uint8_t *)reply,
              sizeof(reply));
  LOG("input=%s %s", source, reply);
  log_summary();
}

static void on_start(ANativeActivity *activity) {
  (void)activity;
  LOG("lifecycle=start");
}

static void on_resume(ANativeActivity *activity) {
  (void)activity;
  LOG("lifecycle=resume");
}

static void on_pause(ANativeActivity *activity) {
  (void)activity;
  LOG("lifecycle=pause");
}

static void on_stop(ANativeActivity *activity) {
  (void)activity;
  LOG("lifecycle=stop");
}

static void on_destroy(ANativeActivity *activity) {
  (void)activity;
  LOG("lifecycle=destroy");
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
    if (type == AINPUT_EVENT_TYPE_MOTION &&
        (AMotionEvent_getAction(event) & AMOTION_EVENT_ACTION_MASK) ==
            AMOTION_EVENT_ACTION_UP) {
      run("add Touch task", "touch");
    } else if (type == AINPUT_EVENT_TYPE_KEY &&
               AKeyEvent_getAction(event) == AKEY_EVENT_ACTION_UP) {
      run("done 2", "key");
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

static AAsset *open_asset(ANativeActivity *activity, const char *name,
                          const uint8_t **bytes, size_t *length) {
  AAsset *asset =
      AAssetManager_open(activity->assetManager, name, AASSET_MODE_BUFFER);
  if (asset == NULL) {
    LOG("asset=missing name=%s", name);
    return NULL;
  }
  *bytes = (const uint8_t *)AAsset_getBuffer(asset);
  *length = (size_t)AAsset_getLength(asset);
  return asset;
}

static void load_board(ANativeActivity *activity) {
  const uint8_t *bytes = NULL;
  size_t length = 0;
  AAsset *asset = open_asset(activity, "board.txt", &bytes, &length);
  if (asset == NULL) {
    return;
  }
  LOG("board=loaded tasks=%d", tasks_load(bytes, length));
  AAsset_close(asset);
  log_summary();
  const char *name = "notes.txt";
  asset = open_asset(activity, name, &bytes, &length);
  if (asset == NULL) {
    return;
  }
  char report[128];
  tasks_asset_report((const uint8_t *)name, strlen(name), bytes, length,
                     (uint8_t *)report, sizeof(report));
  LOG("%s", report);
  AAsset_close(asset);
}

__attribute__((visibility("default"))) void
ANativeActivity_onCreate(ANativeActivity *activity, void *saved_state,
                         size_t saved_state_size) {
  (void)saved_state;
  (void)saved_state_size;
  LOG("lifecycle=create");
  activity->callbacks->onStart = on_start;
  activity->callbacks->onResume = on_resume;
  activity->callbacks->onPause = on_pause;
  activity->callbacks->onStop = on_stop;
  activity->callbacks->onDestroy = on_destroy;
  activity->callbacks->onInputQueueCreated = on_input_queue_created;
  activity->callbacks->onInputQueueDestroyed = on_input_queue_destroyed;
  load_board(activity);
}
