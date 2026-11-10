#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct external_greet_state external_greet_state;

external_greet_state* external_greet_create(const char* name);
const char* external_greet_message(external_greet_state* state);
void external_greet_destroy(external_greet_state* state);

#ifdef __cplusplus
}
#endif
