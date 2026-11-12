/* Runs one Lua script with LuaFileSystem linked in, which LDoc needs and a plain Lua cannot load.
 *
 * usage: ldoc_main LUA_PATH SCRIPT [ARG...]
 */

#include <stdio.h>
#include <stdlib.h>

#include "lauxlib.h"
#include "lua.h"
#include "lualib.h"

int luaopen_lfs(lua_State *state);

static void report(const char *message) {
  (void)fputs("ldoc_main: ", stderr);
  (void)fputs(message, stderr);
  (void)fputc('\n', stderr);
}

int main(int argc, char **argv) {
  if (argc < 3) {
    (void)fputs("usage: ldoc_main LUA_PATH SCRIPT [ARG...]\n", stderr);
    return 2;
  }
  lua_State *state = luaL_newstate();
  if (state == NULL) {
    report("cannot create a Lua state");
    return 1;
  }
  luaL_openlibs(state);
  luaL_requiref(state, "lfs", luaopen_lfs, 0);
  lua_pop(state, 1);
  lua_getglobal(state, "package");
  lua_pushstring(state, argv[1]);
  lua_setfield(state, -2, "path");
  lua_pushliteral(state, "");
  lua_setfield(state, -2, "cpath");
  lua_pop(state, 1);
  lua_createtable(state, argc - 3, 1);
  for (int i = 2; i < argc; i++) {
    lua_pushstring(state, argv[i]);
    lua_rawseti(state, -2, i - 2);
  }
  lua_setglobal(state, "arg");
  if (luaL_loadfile(state, argv[2]) != LUA_OK) {
    report(lua_tostring(state, -1));
    return 1;
  }
  for (int i = 3; i < argc; i++) {
    lua_pushstring(state, argv[i]);
  }
  if (lua_pcall(state, argc - 3, 0, 0) != LUA_OK) {
    report(lua_tostring(state, -1));
    return 1;
  }
  lua_close(state);
  return 0;
}
