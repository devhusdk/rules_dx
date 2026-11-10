#include "external_greet.h"

#include <string>

struct external_greet_state {
  std::string message;
};

external_greet_state* external_greet_create(const char* name) {
  const std::string who = (name != nullptr) ? name : "world";
  external_greet_state* state = new external_greet_state();
  state->message = "Hello from the external native library, " + who;
  return state;
}

const char* external_greet_message(external_greet_state* state) {
  if (state == nullptr) {
    return "";
  }
  return state->message.c_str();
}

void external_greet_destroy(external_greet_state* state) {
  delete state;
}
