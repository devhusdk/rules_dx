"use strict";

const fs = require("fs");
const process = require("process");
const zlib = require("zlib");

function fail(message) {
  process.stdout.write(`::error::${message}\n`);
  process.exit(1);
}

const root = process.env.INPUT_PATH || "";
const needles = [];
if (process.env.BUILDBUDDY_API_KEY) {
  needles.push(Buffer.from(process.env.BUILDBUDDY_API_KEY));
}
const header = process.env.REMOTE_CACHE_HEADER;
if (header) {
  const at = header.indexOf("=");
  const value = at < 0 ? "" : header.slice(at + 1);
  if (value === "") {
    fail("REMOTE_CACHE_HEADER has no value to scan for");
  }
  needles.push(Buffer.from(value));
}
if (needles.length === 0) {
  process.exit(0);
}

try {
  fs.statSync(root);
} catch (error) {
  if (error.code === "ENOENT" || error.code === "ENOTDIR") {
    process.exit(0);
  }
  fail(`scan of ${root} failed`);
}

const seen = new Set();

function holdsSecret(path) {
  const real = fs.realpathSync(path);
  const info = fs.statSync(path);
  if (info.isDirectory()) {
    if (seen.has(real)) {
      return false;
    }
    seen.add(real);
    return fs.readdirSync(path).some((name) => holdsSecret(`${path}/${name}`));
  }
  if (!info.isFile()) {
    return false;
  }
  const raw = fs.readFileSync(path);
  const gzipped = raw.length > 1 && raw[0] === 0x1f && raw[1] === 0x8b;
  const flush = { finishFlush: zlib.constants.Z_SYNC_FLUSH };
  const contents = gzipped ? [raw, zlib.gunzipSync(raw, flush)] : [raw];
  return contents.some((content) => needles.some((needle) => content.includes(needle)));
}

let found;
try {
  found = holdsSecret(root);
} catch (error) {
  fail(`scan of ${root} failed`);
}
if (found) {
  fail(`${root} holds a remote cache secret`);
}
