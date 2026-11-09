/* Runs one Lua script with LuaFileSystem linked in, which LDoc needs and a plain Lua cannot load.
 *
 * usage: ldoc_main LUA_PATH SCRIPT [ARG...]
 */

#include <stdio.h>
#include <stdlib.h>

#include "lauxlib.h"
#include "lua.h"
#include "lualib.h"

int luaopen_lfs(lua_State *L);

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: ldoc_main LUA_PATH SCRIPT [ARG...]\n");
    return 2;
  }
  lua_State *L = luaL_newstate();
  if (L == NULL) {
    fprintf(stderr, "ldoc_main: cannot create a Lua state\n");
    return 1;
  }
  luaL_openlibs(L);
  luaL_requiref(L, "lfs", luaopen_lfs, 0);
  lua_pop(L, 1);
  lua_getglobal(L, "package");
  lua_pushstring(L, argv[1]);
  lua_setfield(L, -2, "path");
  lua_pushliteral(L, "");
  lua_setfield(L, -2, "cpath");
  lua_pop(L, 1);
  lua_createtable(L, argc - 3, 1);
  for (int i = 2; i < argc; i++) {
    lua_pushstring(L, argv[i]);
    lua_rawseti(L, -2, i - 2);
  }
  lua_setglobal(L, "arg");
  if (luaL_loadfile(L, argv[2]) != LUA_OK) {
    fprintf(stderr, "ldoc_main: %s\n", lua_tostring(L, -1));
    return 1;
  }
  for (int i = 3; i < argc; i++) {
    lua_pushstring(L, argv[i]);
  }
  if (lua_pcall(L, argc - 3, 0, 0) != LUA_OK) {
    fprintf(stderr, "ldoc_main: %s\n", lua_tostring(L, -1));
    return 1;
  }
  lua_close(L);
  return 0;
}
