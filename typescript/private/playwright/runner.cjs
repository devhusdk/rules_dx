"use strict";

const { spawnSync } = require("node:child_process");
const net = require("node:net");
const fs = require("node:fs");
const path = require("node:path");
const pins = require("./pins.cjs");

const extra = process.argv.slice(2);
const { DX_PLAYWRIGHT_CONFIG: configPath, DX_PLAYWRIGHT_MARKER: markerPath, DX_PLAYWRIGHT_APP: appPath } = process.env;
const runfiles = process.env.JS_BINARY__RUNFILES || process.env.RUNFILES_DIR;
const resolve = (rlocation) => path.resolve(runfiles, rlocation);

function freePort() {
  return new Promise((done, fail) => {
    const server = net.createServer();
    server.once("error", fail);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => done(port));
    });
  });
}

async function main() {
  const pinned = { revision: process.env.DX_PLAYWRIGHT_CHROMIUM_REVISION, version: process.env.DX_PLAYWRIGHT_VERSION };
  const rejection = pins.mismatch(pinned, pins.installed(process.cwd()));
  if (rejection) {
    console.error(rejection);
    process.exit(1);
  }
  const cli = require.resolve("@playwright/test/cli", { paths: [process.cwd()] });
  const config = resolve(configPath);
  const marker = resolve(markerPath);
  const outputs = process.env.TEST_UNDECLARED_OUTPUTS_DIR;
  const env = {
    ...process.env,
    PLAYWRIGHT_BROWSERS_PATH: path.dirname(path.dirname(marker)),
    PLAYWRIGHT_JUNIT_OUTPUT_FILE: process.env.XML_OUTPUT_FILE,
    DX_PLAYWRIGHT_PORT: String(await freePort()),
  };
  const closure = path.join(path.dirname(path.dirname(path.dirname(marker))), process.env.DX_PLAYWRIGHT_CLOSURE_DIR, "lib");
  if (fs.existsSync(closure)) {
    env.DX_PLAYWRIGHT_CLOSURE = closure;
    env.NODE_OPTIONS = [process.env.NODE_OPTIONS, "--require", JSON.stringify(path.join(__dirname, "chromium_env.cjs"))].filter(Boolean).join(" ");
  }
  if (appPath) {
    env.DX_PLAYWRIGHT_APP = resolve(appPath);
  }
  const args = [cli, "test", "--config", config, "--reporter", "junit,line", "--trace", "retain-on-failure"];
  if (outputs) {
    args.push("--output", path.join(outputs, "playwright"));
  }
  const result = spawnSync(process.execPath, args.concat(extra), { env, stdio: "inherit" });
  process.exit(result.status === null ? 1 : result.status);
}

main();
