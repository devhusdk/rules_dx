#include <erl_nif.h>

static ERL_NIF_TERM probe(ErlNifEnv *env, int argc, const ERL_NIF_TERM argv[]) {
  return enif_make_int(env, 1);
}

static ErlNifFunc funcs[] = {{"probe", 0, probe}};

ERL_NIF_INIT(probe_nif, funcs, NULL, NULL, NULL, NULL)
