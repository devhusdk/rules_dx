#include <erl_nif.h>

#include <stdexcept>
#include <string>
#include <vector>

namespace {

// Each NIF library owns its copy of the C++ runtime, so this counter is per
// library.
int next_call() {
  static int calls = 0;
  return ++calls;
}

ERL_NIF_TERM probe(ErlNifEnv *env, int argc, const ERL_NIF_TERM argv[]) {
  (void)argv;
  if (argc != 0) {
    return enif_make_badarg(env);
  }
  std::vector<std::string> words;
  for (int i = 0; i < 64; ++i) {
    words.push_back(std::string(40, 'a') + std::to_string(i));
  }
  int caught = 0;
  try {
    throw std::runtime_error("dx_cpp_a");
  } catch (const std::runtime_error &e) {
    caught = std::string(e.what()) == "dx_cpp_a" ? 1 : 0;
  }
  return enif_make_tuple3(env, enif_make_int(env, caught),
                          enif_make_int(env, static_cast<int>(words.size())),
                          enif_make_int(env, next_call()));
}

ErlNifFunc funcs[] = {{"probe", 0, probe, 0}};

} // namespace

ERL_NIF_INIT(dx_cpp_a, funcs, NULL, NULL, NULL, NULL)
