#include "deploy/tests/fixtures/consumer_archive/greet.h"

#include <string>

struct consumer_greet_state {
  std::string message;
};

consumer_greet_state* consumer_greet_create(const char* name) {
  const std::string who = (name != nullptr) ? name : "world";
  consumer_greet_state* state = new consumer_greet_state();
  state->message = "Hello from the consumer native library, " + who;
  return state;
}

const char* consumer_greet_message(consumer_greet_state* state) {
  if (state == nullptr) {
    return "";
  }
  return state->message.c_str();
}

void consumer_greet_destroy(consumer_greet_state* state) {
  delete state;
}
