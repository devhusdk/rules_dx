"use strict";

const assert = require("node:assert/strict");
const childProcess = require("node:child_process");
const path = require("node:path");

const runfiles = process.env.JS_BINARY__RUNFILES || process.env.RUNFILES_DIR;
const browsers = path.resolve("browsers");
const browser = path.join(
	browsers,
	"chromium_headless_shell-1",
	"chrome-headless-shell",
);
const other = path.resolve("bin", "server");

const calls = [];
childProcess.spawn = (command, args, options) => {
	calls.push({ command, args, options });
};

process.env.PLAYWRIGHT_BROWSERS_PATH = browsers;
process.env.DX_PLAYWRIGHT_CLOSURE = "/dx_closure/lib";
process.env.LD_LIBRARY_PATH = "/dx_inherited";
require(path.resolve(runfiles, process.env.DX_PLAYWRIGHT_CHROMIUM_ENV));

childProcess.spawn(browser, ["--headless"], {
	env: { HOME: "/home", LD_LIBRARY_PATH: "/dx_given" },
	stdio: "pipe",
});
childProcess.spawn(browser, ["--headless"]);
childProcess.spawn(browser, { stdio: "pipe" });
childProcess.spawn(browser);
childProcess.spawn(other, ["--port"], {
	env: { LD_LIBRARY_PATH: "/dx_given" },
});
childProcess.spawn(other);

const inherited = { ...process.env, LD_LIBRARY_PATH: "/dx_closure/lib" };
assert.deepEqual(calls, [
	{
		command: browser,
		args: ["--headless"],
		options: {
			env: { HOME: "/home", LD_LIBRARY_PATH: "/dx_closure/lib" },
			stdio: "pipe",
		},
	},
	{ command: browser, args: ["--headless"], options: { env: inherited } },
	{ command: browser, args: [], options: { env: inherited, stdio: "pipe" } },
	{ command: browser, args: [], options: { env: inherited } },
	{
		command: other,
		args: ["--port"],
		options: { env: { LD_LIBRARY_PATH: "/dx_given" } },
	},
	{ command: other, args: undefined, options: undefined },
]);
