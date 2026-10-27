#include <jni.h>

extern "C" JNIEXPORT jstring JNICALL
Java_org_example_adoptandroid_NativeBridge_greeting(JNIEnv* env, jclass) {
  return env->NewStringUTF("hello from native");
}
