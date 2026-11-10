"use strict";

const fs = require("node:fs");
const path = require("node:path");

const SHELL = "chromium-headless-shell";

function installed(from) {
  const test = require.resolve("@playwright/test/package.json", { paths: [from] });
  const playwright = require.resolve("playwright/package.json", { paths: [path.dirname(test)] });
  const core = require.resolve("playwright-core/package.json", { paths: [path.dirname(playwright)] });
  const browsers = JSON.parse(fs.readFileSync(path.join(path.dirname(core), "browsers.json"), "utf8")).browsers;
  const shell = browsers.find((browser) => browser.name === SHELL);
  return {
    version: JSON.parse(fs.readFileSync(test, "utf8")).version,
    revision: shell ? shell.revision : undefined,
    browserVersion: shell ? shell.browserVersion : undefined,
  };
}

function mismatch(pinned, found) {
  if (found.revision === pinned.revision) {
    return null;
  }
  return (
    `typescript_playwright_test pins Chromium headless shell revision ${pinned.revision}, ` +
    `but the installed @playwright/test ${found.version} needs revision ${found.revision}; ` +
    `install @playwright/test ${pinned.version}`
  );
}

module.exports = { installed, mismatch };
