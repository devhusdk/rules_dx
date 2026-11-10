"use strict";

const { spawnSync } = require("node:child_process");
const net = require("node:net");
const path = require("node:path");

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
  const cli = require.resolve("@playwright/test/cli", { paths: [process.cwd()] });
  const config = resolve(configPath);
  const outputs = process.env.TEST_UNDECLARED_OUTPUTS_DIR;
  const env = {
    ...process.env,
    PLAYWRIGHT_BROWSERS_PATH: path.dirname(path.dirname(resolve(markerPath))),
    PLAYWRIGHT_JUNIT_OUTPUT_FILE: process.env.XML_OUTPUT_FILE,
    DX_PLAYWRIGHT_PORT: String(await freePort()),
  };
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
