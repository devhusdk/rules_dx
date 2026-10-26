// Runs one Lua program from the <name>.luaentry file next to the executable.

#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "lua.hpp"
#include "rules_cc/cc/runfiles/runfiles.h"

namespace {

using rules_cc::cc::runfiles::Runfiles;

constexpr char kEntrySuffix[] = ".luaentry";
constexpr char kModulesKey[] = "dx.lua.modules";

struct Entry {
  std::string main;
  std::string source_repo;
  std::map<std::string, std::string> modules;
};

std::unique_ptr<Runfiles> g_runfiles;
std::string g_source_repo;

[[noreturn]] void Die(const std::string &message) {
  std::fprintf(stderr, "lua: %s\n", message.c_str());
  std::exit(1);
}

bool EndsWithExe(const std::string &path) {
#ifdef _WIN32
  if (path.size() < 4)
    return false;
  std::string tail = path.substr(path.size() - 4);
  for (char &c : tail)
    c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
  return tail == ".exe";
#else
  (void)path;
  return false;
#endif
}

std::string EntryPath(const std::string &argv0) {
  std::string base = argv0;
  if (EndsWithExe(base))
    base.resize(base.size() - 4);
  return base + kEntrySuffix;
}

bool ParseEntry(const std::string &text, Entry *entry, std::string *error) {
  std::istringstream in(text);
  std::string line;
  int number = 0;
  while (std::getline(in, line)) {
    ++number;
    if (!line.empty() && line.back() == '\r')
      line.pop_back();
    if (number == 1) {
      entry->main = line;
      continue;
    }
    if (number == 2) {
      entry->source_repo = line;
      continue;
    }
    if (line.empty())
      continue;
    size_t tab = line.find('\t');
    if (tab == std::string::npos || tab == 0 || tab + 1 == line.size()) {
      *error =
          "line " + std::to_string(number) + " is not <module>\\t<rlocation>";
      return false;
    }
    entry->modules[line.substr(0, tab)] = line.substr(tab + 1);
  }
  if (number < 2 || entry->main.empty()) {
    *error = "missing the main rlocation or the source repository line";
    return false;
  }
  return true;
}

Entry LoadEntry(const std::string &argv0) {
  std::string path = EntryPath(argv0);
  std::ifstream file(path, std::ios::binary);
  if (!file)
    Die("cannot open entry file " + path);
  std::stringstream text;
  text << file.rdbuf();
  Entry entry;
  std::string error;
  if (!ParseEntry(text.str(), &entry, &error))
    Die(path + ": " + error);
  return entry;
}

std::string Resolve(const std::string &rlocation) {
  return g_runfiles->Rlocation(rlocation, g_source_repo);
}

bool Readable(const std::string &path) {
  std::ifstream file(path, std::ios::binary);
  return static_cast<bool>(file);
}

int MapSearcher(lua_State *L) {
  const char *name = luaL_checkstring(L, 1);
  lua_getfield(L, LUA_REGISTRYINDEX, kModulesKey);
  lua_getfield(L, -1, name);
  if (lua_isnil(L, -1)) {
    lua_pushfstring(L, "no module '%s' in the deps of this target", name);
    return 1;
  }
  std::string rlocation = lua_tostring(L, -1);
  std::string path = Resolve(rlocation);
  if (path.empty() || !Readable(path)) {
    lua_pushfstring(L, "module '%s' maps to %s, which is not in the runfiles",
                    name, rlocation.c_str());
    return 1;
  }
  if (luaL_loadfilex(L, path.c_str(), "t") != LUA_OK) {
    return luaL_error(L, "error loading module '%s' from file '%s':\n\t%s",
                      name, path.c_str(), lua_tostring(L, -1));
  }
  lua_pushstring(L, path.c_str());
  return 2;
}

int RunfilesRlocation(lua_State *L) {
  const char *key = luaL_checkstring(L, 1);
  std::string path = Resolve(key);
  if (path.empty()) {
    lua_pushnil(L);
  } else {
    lua_pushstring(L, path.c_str());
  }
  return 1;
}

int OpenRunfiles(lua_State *L) {
  lua_newtable(L);
  lua_pushcfunction(L, RunfilesRlocation);
  lua_setfield(L, -2, "rlocation");
  return 1;
}

void ConfigurePackage(lua_State *L, const Entry &entry) {
  lua_newtable(L);
  for (const auto &[module, rlocation] : entry.modules) {
    lua_pushstring(L, rlocation.c_str());
    lua_setfield(L, -2, module.c_str());
  }
  lua_setfield(L, LUA_REGISTRYINDEX, kModulesKey);

  lua_getglobal(L, "package");
  lua_pushliteral(L, "");
  lua_setfield(L, -2, "path");
  lua_pushliteral(L, "");
  lua_setfield(L, -2, "cpath");

  lua_getfield(L, -1, "searchers");
  lua_rawgeti(L, -1, 1);
  lua_newtable(L);
  lua_insert(L, -2);
  lua_rawseti(L, -2, 1);
  lua_pushcfunction(L, MapSearcher);
  lua_rawseti(L, -2, 2);
  lua_setfield(L, -3, "searchers");
  lua_pop(L, 1);

  lua_getfield(L, -1, "preload");
  lua_pushcfunction(L, OpenRunfiles);
  lua_setfield(L, -2, "runfiles");
  lua_pop(L, 2);
}

int MessageHandler(lua_State *L) {
  const char *message = lua_tostring(L, 1);
  if (message == nullptr) {
    if (luaL_callmeta(L, 1, "__tostring") && lua_type(L, -1) == LUA_TSTRING)
      return 1;
    message =
        lua_pushfstring(L, "(error object is a %s value)", luaL_typename(L, 1));
  }
  luaL_traceback(L, L, message, 1);
  return 1;
}

void SetArgs(lua_State *L, int argc, char **argv, const std::string &script) {
  lua_createtable(L, argc, 2);
  lua_pushstring(L, argv[0]);
  lua_rawseti(L, -2, -1);
  lua_pushstring(L, script.c_str());
  lua_rawseti(L, -2, 0);
  for (int i = 1; i < argc; ++i) {
    lua_pushstring(L, argv[i]);
    lua_rawseti(L, -2, i);
  }
  lua_setglobal(L, "arg");
}

} // namespace

int main(int argc, char **argv) {
  if (argc < 1 || argv[0] == nullptr || argv[0][0] == '\0')
    Die("argv[0] is empty");
  Entry entry = LoadEntry(argv[0]);
  g_source_repo = entry.source_repo;

  std::string error;
  g_runfiles.reset(Runfiles::Create(argv[0], entry.source_repo, &error));
  if (g_runfiles == nullptr)
    Die("cannot open runfiles: " + error);

  std::string script = Resolve(entry.main);
  if (script.empty() || !Readable(script))
    Die("main chunk " + entry.main + " is not in the runfiles");

  lua_State *L = luaL_newstate();
  if (L == nullptr)
    Die("cannot create a Lua state");
  lua_pushboolean(L, 1);
  lua_setfield(L, LUA_REGISTRYINDEX, "LUA_NOENV");
  luaL_openlibs(L);
  ConfigurePackage(L, entry);
  SetArgs(L, argc, argv, script);

  lua_pushcfunction(L, MessageHandler);
  int handler = lua_gettop(L);
  if (luaL_loadfilex(L, script.c_str(), "t") != LUA_OK) {
    std::fprintf(stderr, "lua: %s\n", lua_tostring(L, -1));
    lua_close(L);
    return 1;
  }
  for (int i = 1; i < argc; ++i)
    lua_pushstring(L, argv[i]);
  int status = lua_pcall(L, argc - 1, 0, handler);
  if (status != LUA_OK) {
    std::fprintf(stderr, "lua: %s\n", lua_tostring(L, -1));
    std::fflush(stderr);
    lua_close(L);
    return 1;
  }
  lua_close(L);
  return 0;
}
