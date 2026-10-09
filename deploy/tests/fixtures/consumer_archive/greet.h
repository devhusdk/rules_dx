#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct consumer_greet_state consumer_greet_state;

consumer_greet_state* consumer_greet_create(const char* name);
const char* consumer_greet_message(consumer_greet_state* state);
void consumer_greet_destroy(consumer_greet_state* state);

#ifdef __cplusplus
}
#endif
