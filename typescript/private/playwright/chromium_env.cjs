"use strict";

const childProcess = require("node:child_process");
const { syncBuiltinESMExports } = require("node:module");
const path = require("node:path");

const { PLAYWRIGHT_BROWSERS_PATH: browsers, DX_PLAYWRIGHT_CLOSURE: closure } = process.env;

if (browsers && closure) {
  const spawn = childProcess.spawn;
  childProcess.spawn = function (command, args, options) {
    if (typeof command !== "string" || !command.startsWith(browsers + path.sep)) {
      return spawn.apply(this, arguments);
    }
    const given = Array.isArray(args) ? options : args;
    const env = { ...((given && given.env) || process.env), LD_LIBRARY_PATH: closure };
    return spawn.call(this, command, Array.isArray(args) ? args : [], { ...given, env });
  };
  syncBuiltinESMExports();
}
