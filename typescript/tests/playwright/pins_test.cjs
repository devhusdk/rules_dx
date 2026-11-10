"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const runfiles = process.env.JS_BINARY__RUNFILES || process.env.RUNFILES_DIR;
const resolve = (rlocation) => path.resolve(runfiles, rlocation);
const pins = require(resolve(process.env.DX_PLAYWRIGHT_PINS));

const pinned = {
	browserVersion: process.env.DX_PLAYWRIGHT_CHROMIUM_VERSION,
	revision: process.env.DX_PLAYWRIGHT_CHROMIUM_REVISION,
	version: process.env.DX_PLAYWRIGHT_VERSION,
};
const found = pins.installed(process.cwd());

assert.deepEqual(found, pinned);
assert.equal(pins.mismatch(pinned, found), null);
assert.equal(
	pins.mismatch(pinned, { revision: "9999", version: "9.9.9" }),
	`typescript_playwright_test pins Chromium headless shell revision ${pinned.revision}, ` +
		"but the installed @playwright/test 9.9.9 needs revision 9999; " +
		`install @playwright/test ${pinned.version}`,
);

const declared = JSON.parse(
	fs.readFileSync(resolve(process.env.DX_PACKAGE_JSON), "utf8"),
);
assert.equal(declared.devDependencies["@playwright/test"], pinned.version);

const readme = fs.readFileSync(resolve(process.env.DX_README), "utf8");
assert.ok(
	readme.includes(`\`@playwright/test\` ${pinned.version} `),
	"README names the pinned @playwright/test version",
);
assert.ok(!readme.includes("libnss3"), "README names no host library");
