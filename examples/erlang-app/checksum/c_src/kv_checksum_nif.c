#include <erl_nif.h>

#include "examples/erlang-app/checksum/c_src/fnv1a.h"

static ERL_NIF_TERM fnv1a(ErlNifEnv *env, int argc, const ERL_NIF_TERM argv[]) {
  ErlNifBinary data;
  if (argc != 1 || !enif_inspect_binary(env, argv[0], &data)) {
    return enif_make_badarg(env);
  }
  return enif_make_uint(env, fnv1a_32(data.data, data.size));
}

static ErlNifFunc funcs[] = {{"fnv1a", 1, fnv1a, 0}};

ERL_NIF_INIT(kv_checksum, funcs, NULL, NULL, NULL, NULL)
