#include <jni.h>

#include <string>

#include "summary.h"

extern "C" JNIEXPORT jstring JNICALL
Java_org_example_adoptandroid_NativeBridge_summarize(JNIEnv *env, jclass,
                                                     jstring text) {
  const char *chars = env->GetStringUTFChars(text, nullptr);
  std::string input(chars == nullptr ? "" : chars);
  if (chars != nullptr) {
    env->ReleaseStringUTFChars(text, chars);
  }
  return env->NewStringUTF(adopt::Summarize(input).c_str());
}
