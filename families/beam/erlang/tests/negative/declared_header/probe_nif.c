#include "erlang/tests/negative/declared_header/hidden.h"
#include <erl_nif.h>

static ERL_NIF_TERM probe(ErlNifEnv *env, int argc, const ERL_NIF_TERM argv[]) {
  (void)argc;
  (void)argv;
  return enif_make_int(env, HIDDEN);
}

static ErlNifFunc funcs[] = {{"probe", 0, probe, 0}};

ERL_NIF_INIT(probe, funcs, NULL, NULL, NULL, NULL)
