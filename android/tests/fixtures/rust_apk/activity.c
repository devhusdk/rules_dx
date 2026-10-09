#include <stddef.h>
#include <stdint.h>

#ifdef __ANDROID__
#include <android/native_activity.h>
#else
typedef struct ANativeActivity ANativeActivity;

struct ANativeActivity {
    void *instance;
};
#endif

extern int app_main(void);

void ANativeActivity_onCreate(ANativeActivity *activity, void *saved_state, size_t saved_state_size) {
    (void)saved_state;
    (void)saved_state_size;
    activity->instance = (void *)(intptr_t)app_main();
}
