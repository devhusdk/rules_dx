// Runs Jazzer.js on the fuzz target in argv[2]; the remaining arguments go to libFuzzer.
const { createRequire } = require("node:module");
const path = require("node:path");

const target = process.argv[2];
if (!target) {
  console.error("usage: fuzz_driver.cjs <fuzz target> [libFuzzer arguments]");
  process.exit(2);
}

const cli = createRequire(path.resolve(target)).resolve("@jazzer.js/core/dist/cli.js");
process.argv = [process.argv[0], cli, target, "--", ...process.argv.slice(3)];
require(cli);
